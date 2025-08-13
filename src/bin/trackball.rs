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
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::main;
use esp_hal::time::{Duration, Instant};
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
    let delay = Delay::new();

    // turn on the board
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    // set up the trackball button pin
    let tdeck_track_click = Input::new(
        peripherals.GPIO0,
        InputConfig::default().with_pull(Pull::Up),
    );
    let tdeck_trackball_right = Input::new(
        peripherals.GPIO2,
        InputConfig::default().with_pull(Pull::Up),
    );
    let tdeck_trackball_left = Input::new(
        peripherals.GPIO1,
        InputConfig::default().with_pull(Pull::Up),
    );
    let tdeck_trackball_up = Input::new(
        peripherals.GPIO3,
        InputConfig::default().with_pull(Pull::Up),
    );
    let tdeck_trackball_down = Input::new(
        peripherals.GPIO15,
        InputConfig::default().with_pull(Pull::Up),
    );
    let mut last_right_high = false;
    let mut last_left_high = false;
    let mut last_up_high = false;
    let mut last_down_high = false;

    info!("running");
    loop {
        // info!("button pressed is {} ", tdeck_track_click.is_low());
        if tdeck_trackball_right.is_high() != last_right_high {
            info!("trackball right changed ");
            last_right_high = tdeck_trackball_right.is_high();
        }
        if tdeck_trackball_left.is_high() != last_left_high {
            info!("trackball left changed ");
            last_left_high = tdeck_trackball_left.is_high();
        }
        if tdeck_trackball_up.is_high() != last_up_high {
            info!("trackball up changed ");
            last_up_high = tdeck_trackball_up.is_high();
        }
        if tdeck_trackball_down.is_high() != last_down_high {
            info!("trackball down changed ");
            last_down_high = tdeck_trackball_down.is_high();
        }
        // wait one msec
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(1) {}
    }
}
