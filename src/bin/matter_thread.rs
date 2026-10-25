#![no_std]
#![no_main]
#![recursion_limit = "256"]

use defmt::info;
use embassy_executor::Spawner;
use esp_alloc::heap_allocator;
use esp_backtrace as _;
use esp_hal::{ram, rng::TrngSource, timer::timg::TimerGroup};
use esp_metadata_generated::memory_range;
use esp_println as _;
use rs_matter_embassy::{
    matter::{
        BasicCommData, clusters,
        crypto::{Crypto, Rng, default_crypto},
        devices,
        dm::{
            Async, Dataver, EmptyHandler, Endpoint, Node,
            clusters::{
                app::on_off::{self, OnOffHooks, test::TestOnOffDeviceLogic},
                basic_info::BasicInfoConfig,
                desc::{self, ClusterHandler as _},
            },
            devices::{
                DEV_TYPE_ON_OFF_LIGHT,
                test::{DAC_PRIVKEY, TEST_DEV_ATT, TEST_DEV_COMM, TEST_DEV_DET},
            },
            endpoints::ROOT_ENDPOINT_ID,
        },
        persist::DummyKvBlobStore,
        utils::init::InitMaybeUninit,
    },
    stack::rand::reseeding_csprng,
    wireless::{EmbassyThread, EmbassyThreadMatterStack, esp::EspThreadDriver},
};
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

const BASIC_INFO: BasicInfoConfig = BasicInfoConfig {
    sai: Some(500),
    ..TEST_DEV_DET
};

const NODE: Node = Node {
    endpoints: &[
        MatterStack::root_endpoint(),
        Endpoint::new(
            LIGHT_ENDPOINT,
            devices!(DEV_TYPE_ON_OFF_LIGHT),
            clusters!(desc::DescHandler::CLUSTER, TestOnOffDeviceLogic::CLUSTER),
        ),
    ],
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    info!("Matter/Thread experimental baseline: no LED control or persistent pairing");

    // Recover the extra RAM region as well as the normal heap, as in the upstream ESP example.
    heap_allocator!(size: HEAP_SIZE - RECLAIMED_RAM);
    heap_allocator!(#[ram(reclaimed)] size: RECLAIMED_RAM);

    let peripherals = esp_hal::init(esp_hal::Config::default());
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
    let mut ieee_eui64 = [0; 8];
    random.fill_bytes(&mut ieee_eui64);
    ieee_eui64[0] = (ieee_eui64[0] | 0x02) & !0x01;
    let discriminator = (random.next_u32() & 0x0fff) as u16;

    let timers = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timers.timer0, peripherals.FROM_CPU_INTR0);

    let stack = STACK.uninit().init_with(MatterStack::init(
        &BASIC_INFO,
        BasicCommData {
            password: TEST_DEV_COMM.password,
            discriminator,
        },
        &TEST_DEV_ATT,
    ));
    let on_off = on_off::OnOffHandler::new_standalone(
        Dataver::new_rand(&mut random),
        LIGHT_ENDPOINT,
        TestOnOffDeviceLogic::new(false),
    );
    let handler = EmptyHandler
        .chain(
            |endpoint, _| endpoint == ROOT_ENDPOINT_ID,
            Async(MatterStack::root_handler(&(), &mut random)),
        )
        .chain(
            |endpoint, cluster| {
                endpoint == LIGHT_ENDPOINT && cluster == TestOnOffDeviceLogic::CLUSTER.id
            },
            on_off::HandlerAsyncAdaptor(&on_off),
        )
        .chain(
            |endpoint, cluster| {
                endpoint == LIGHT_ENDPOINT && cluster == desc::DescHandler::CLUSTER.id
            },
            Async(desc::DescHandler::new(Dataver::new_rand(&mut random)).adapt()),
        );

    // Intentionally volatile for this build-only milestone: never claim reboot-safe pairing.
    let mut store = DummyKvBlobStore;
    stack
        .startup(&crypto, &mut store)
        .await
        .expect("Failed to initialize Matter storage");
    let kv = stack.matter().kv(store);
    stack
        .run(
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
        )
        .await
        .expect("Matter/Thread stack stopped unexpectedly");
}
