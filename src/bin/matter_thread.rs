#![no_std]
#![no_main]
#![recursion_limit = "256"]

use defmt::{info, warn};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_executor::Spawner;
use embassy_futures::select::{Either, Either3, select, select3};
use embassy_time::Timer;
use embedded_storage::nor_flash::ReadNorFlash;
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_bootloader_esp_idf::partitions::{
    DataPartitionSubType, PARTITION_TABLE_MAX_LEN, PartitionType, read_partition_table,
};
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Pull},
    ram,
    rmt::Rmt,
    rng::TrngSource,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_hal_smartled::{RmtSmartLeds, WS2812B_TIMING, buffer_size, color_order};
use esp_metadata_generated::memory_range;
use esp_println as _;
use esp_storage::FlashStorage;
use esp32_c6_rust::{
    light::device_identity,
    matter_light::{BoardLight, LIGHT_CLUSTER},
};
use rs_matter_embassy::{
    matter::{
        BasicCommData, clusters,
        crypto::{Crypto, default_crypto},
        devices,
        dm::{
            Async, Dataver, EmptyHandler, Endpoint, Node,
            clusters::{
                app::on_off,
                basic_info::BasicInfoConfig,
                desc::{self, ClusterHandler as _},
            },
            devices::{
                DEV_TYPE_ON_OFF_LIGHT,
                test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_DEV_COMM, TEST_DEV_DET},
            },
            endpoints::ROOT_ENDPOINT_ID,
        },
        utils::init::InitMaybeUninit,
    },
    persist::SeqMapKvBlobStore,
    stack::rand::reseeding_csprng,
    wireless::{EmbassyThread, EmbassyThreadMatterStack, esp::EspThreadDriver},
};
use smart_leds::{RGB8, SmartLedsWrite};
use static_cell::StaticCell;
use tinyrlibc as _;

extern crate alloc;

const BUMP_SIZE: usize = 25_000;
const HEAP_SIZE: usize = 100 * 1024;
const RECLAIMED_RAM: usize =
    memory_range!("DRAM2_UNINIT").end - memory_range!("DRAM2_UNINIT").start;
const LIGHT_ENDPOINT: u16 = 1;
type MatterStack = EmbassyThreadMatterStack<'static, BUMP_SIZE, ()>;
static STACK: StaticCell<MatterStack> = StaticCell::new();
static DEVICE_NAME: StaticCell<[u8; 20]> = StaticCell::new();
static BASIC_INFO: StaticCell<BasicInfoConfig<'static>> = StaticCell::new();

const NODE: Node = Node {
    endpoints: &[
        MatterStack::root_endpoint(),
        Endpoint::new(
            LIGHT_ENDPOINT,
            devices!(DEV_TYPE_ON_OFF_LIGHT),
            clusters!(desc::DescHandler::CLUSTER, LIGHT_CLUSTER),
        ),
    ],
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    info!("Starting Matter/Thread onboard blinking light");

    // Recover the extra RAM region as well as the normal heap, as in the upstream ESP example.
    heap_allocator!(size: HEAP_SIZE - RECLAIMED_RAM);
    heap_allocator!(#[ram(reclaimed)] size: RECLAIMED_RAM);

    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));
    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);
    let mut boot = Input::new(
        peripherals.GPIO9,
        InputConfig::default().with_pull(Pull::Up),
    );
    let rmt_frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, rmt_frequency).expect("Failed to initialize RMT");
    let mut led = RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Grb>::new(
        WS2812B_TIMING,
        rmt.channel0,
        peripherals.GPIO8,
        rmt_frequency,
    )
    .expect("Failed to initialize onboard LED");
    led.write([RGB8::new(0, 0, 0)])
        .expect("Failed to turn off onboard LED");

    let mac: [u8; 6] = esp_hal::efuse::base_mac_address()
        .as_bytes()
        .try_into()
        .expect("Expected six-byte base MAC address");
    let (ieee_eui64, name, discriminator) = device_identity(mac);
    let name = core::str::from_utf8(DEVICE_NAME.init(name)).expect("Invalid device identity");
    info!("Device identity: {}", name);
    let basic_info = BASIC_INFO.init(BasicInfoConfig {
        product_name: "ESP32-C6 Blink",
        device_name: "ESP32-C6 Blink",
        serial_no: &name[8..],
        unique_id: name,
        sai: Some(500),
        ..TEST_DEV_DET
    });
    let _entropy = TrngSource::new(peripherals.RNG, peripherals.ADC1);
    let crypto = default_crypto(
        reseeding_csprng(
            esp_hal::rng::Trng::try_new().expect("Failed to initialize hardware entropy"),
            1000,
        )
        .expect("Failed to seed Matter RNG"),
        DAC_PRIVKEY,
    );
    let mut random = crypto.weak_rand().expect("Failed to create metadata RNG");

    let stack = STACK.uninit().init_with(MatterStack::init(
        basic_info,
        BasicCommData {
            password: TEST_DEV_COMM.password,
            discriminator,
        },
        &TEST_DEV_ATT,
    ));
    let mut flash = FlashStorage::new(peripherals.FLASH);
    let range = storage_range(&mut flash);
    if boot.is_low() {
        warn!("BOOT is low at startup; release now or hold for 5 seconds to erase pairing");
        if matches!(
            select(boot.wait_for_high(), Timer::after_secs(5)).await,
            Either::Second(())
        ) {
            erase_pairing(&mut flash, range.clone());
            boot.wait_for_high().await;
        }
    }

    {
        let mut store = SeqMapKvBlobStore::new(BlockingAsync::new(&mut flash), range.clone());
        stack
            .startup(&crypto, &mut store)
            .await
            .expect("Failed to load Matter pairing; inspect storage or deliberately factory-reset");
        info!(
            "Stored Matter fabric present: {}",
            stack.matter().has_fabrics()
        );
        let kv = stack.matter().kv(&mut store);
        let light = BoardLight::load(&kv).expect("Failed to load persistent LED settings");
        let on_off = on_off::OnOffHandler::new_standalone(
            Dataver::new_rand(&mut random),
            LIGHT_ENDPOINT,
            &light,
        );
        let handler = EmptyHandler
            .chain(
                |endpoint, _| endpoint == ROOT_ENDPOINT_ID,
                Async(MatterStack::root_handler(&(), &mut random)),
            )
            .chain(
                |endpoint, cluster| endpoint == LIGHT_ENDPOINT && cluster == LIGHT_CLUSTER.id,
                on_off::HandlerAsyncAdaptor(&on_off),
            )
            .chain(
                |endpoint, cluster| {
                    endpoint == LIGHT_ENDPOINT && cluster == desc::DescHandler::CLUSTER.id
                },
                Async(desc::DescHandler::new(Dataver::new_rand(&mut random)).adapt()),
            );
        info!("Hold BOOT (GPIO9) for 5 seconds while running to factory-reset pairing");
        let matter = stack.run(
            EmbassyThread::new(
                EspThreadDriver::new(peripherals.IEEE802154, peripherals.BT),
                crypto.rand().expect("Failed to create radio RNG"),
                ieee_eui64,
                &kv,
                stack,
                true,
            ),
            &crypto,
            (NODE, handler),
            &kv,
            (),
        );
        match select3(matter, light.run(&mut led), wait_factory_reset(&mut boot)).await {
            Either3::First(result) => {
                result.expect("Matter/Thread stack failed");
                panic!("Matter/Thread stack stopped unexpectedly");
            }
            Either3::Second(()) => panic!("LED task stopped unexpectedly"),
            Either3::Third(()) => {}
        }
    }
    led.write([RGB8::new(0, 0, 0)])
        .expect("Failed to turn off LED before factory reset");
    erase_pairing(&mut flash, range);
    info!("Release BOOT to restart with fresh pairing");
    boot.wait_for_high().await;
    esp_hal::system::software_reset();
}

fn storage_range(flash: &mut FlashStorage<'_>) -> core::ops::Range<u32> {
    let mut buffer = [0; PARTITION_TABLE_MAX_LEN];
    let table = read_partition_table(flash, &mut buffer).expect("Failed to read partition table");
    let partition = table
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .expect("Invalid partition table")
        .expect("Missing NVS partition: refusing to use unreserved flash");
    let start = partition.offset();
    let end = start
        .checked_add(partition.len())
        .expect("NVS range overflow");
    assert!(!partition.is_read_only(), "NVS partition is read-only");
    assert!(
        !partition.is_encrypted(),
        "Encrypted NVS is not supported by this storage adapter"
    );
    assert!(
        start >= 0x9000 && end as usize <= flash.capacity(),
        "NVS out of flash bounds"
    );
    assert!(partition.len() >= 0x6000, "NVS requires at least 24 KiB");
    assert!(
        start.is_multiple_of(FlashStorage::ERASE_SIZE as u32)
            && end.is_multiple_of(FlashStorage::ERASE_SIZE as u32),
        "NVS must be erase-sector aligned"
    );
    let mut entries_at_start = 0;
    for index in 0..table.len() {
        let other = table.get_partition(index).expect("Invalid partition entry");
        if other.offset() == start {
            entries_at_start += 1;
        } else {
            let other_end = other
                .offset()
                .checked_add(other.len())
                .expect("Partition overflow");
            assert!(
                other_end <= start || other.offset() >= end,
                "NVS overlaps another partition"
            );
        }
    }
    assert_eq!(entries_at_start, 1, "Duplicate partition at NVS offset");
    info!(
        "Matter storage: {} at {=u32:#x}..{=u32:#x}",
        partition.label_as_str(),
        start,
        end
    );
    start..end
}

fn erase_pairing(flash: &mut FlashStorage<'_>, range: core::ops::Range<u32>) {
    warn!("Factory reset: erasing only the reserved NVS partition");
    flash
        .erase(range.start, range.end)
        .expect("Failed to erase pairing storage");
}

async fn wait_factory_reset(boot: &mut Input<'_>) {
    loop {
        boot.wait_for_low().await;
        Timer::after_millis(50).await;
        if boot.is_low() {
            warn!("Keep BOOT pressed for 5 seconds to erase pairing; release to cancel");
            if matches!(
                select(boot.wait_for_high(), Timer::after_secs(5)).await,
                Either::Second(())
            ) {
                return;
            }
            info!("Factory reset cancelled");
        }
    }
}
