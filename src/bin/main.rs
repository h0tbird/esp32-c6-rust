#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use defmt::info;
use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::main;
use esp_hal::rmt::Rmt;
use esp_hal::time::Rate;
use esp_hal_smartled::{RmtSmartLeds, WS2812B_TIMING, buffer_size, color_order};
use esp_println as _;
use smart_leds::{RGB8, SmartLedsWrite};

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

#[allow(
    clippy::large_stack_frames,
    reason = "it's not unusual to allocate larger buffers etc. in main"
)]
#[main]
fn main() -> ! {
    // generator version: 1.4.0
    // generator parameters: -o esp32c6 -o unstable-hal -o defmt -o esp-backtrace -o vscode

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let rmt_frequency = Rate::from_mhz(80);
    let rmt = Rmt::new(peripherals.RMT, rmt_frequency).expect("Failed to initialize RMT");
    // One WS2812-compatible RGB LED on GPIO8, with GRB wire order.
    let mut led = RmtSmartLeds::<{ buffer_size::<RGB8>(1) }, _, RGB8, color_order::Grb>::new(
        WS2812B_TIMING,
        rmt.channel0,
        peripherals.GPIO8,
        rmt_frequency,
    )
    .expect("Failed to initialize the LED driver");
    let delay = Delay::new();

    // Keep brightness low: each channel ranges from 0 to 255.
    const LEVEL: u8 = 10;
    let colors = [
        ("red", RGB8::new(LEVEL, 0, 0)),
        ("green", RGB8::new(0, LEVEL, 0)),
        ("cyan", RGB8::new(0, LEVEL, LEVEL)),
        ("blue", RGB8::new(0, 0, LEVEL)),
        ("magenta", RGB8::new(LEVEL, 0, LEVEL)),
    ];

    loop {
        for (name, color) in colors {
            info!("LED: {}", name);
            led.write([color]).expect("Failed to write LED color");
            delay.delay_millis(500);
            led.write([RGB8::new(0, 0, 0)])
                .expect("Failed to turn off LED");
            delay.delay_millis(500);
        }
    }
}
