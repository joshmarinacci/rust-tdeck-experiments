#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use esp_hal::main;
use esp_hal::time::{Rate};
use gt911::{Gt911Blocking};
use log::info;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("running");

    let delay = Delay::new();

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    info!("board is powering up");
    delay.delay_millis(1000);


    let mut i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)).with_timeout(BusTimeout::Disabled),
    )
        .unwrap()
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO8);

    let touch = Gt911Blocking::default();
    touch.init(&mut i2c).unwrap();
    loop {
        if let Ok(points) = touch.get_multi_touch(&mut i2c) {
            // stack allocated Vec containing 0-5 points
            info!("{:?}", points)
        }
    }
    // loop {
    //     if let Ok(point) = touch.get_touch(&mut i2c) {
    //         // point can be Some (pressed or moved) or None (released)
    //         info!("{:?}", point)
    //     } else {
    //         // ignore because nothing has happened since last poll => Error::NotReady
    //     }
    // }


}