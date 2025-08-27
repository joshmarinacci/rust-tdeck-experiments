#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use esp_hal::peripherals::{GPIO4, ADC1};
use alloc::string::String;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::prelude::DrawTarget;
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::{main, Blocking};
use esp_hal::analog::adc::{Adc, AdcConfig, AdcPin, Attenuation};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use esp_hal::peripherals::{Peripherals};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::{Rate};
use log::info;
use mipidsi::{Builder, Display, NoResetPin};
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use static_cell::StaticCell;
use rust_tdeck_experiments::Wrapper;

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
    let mut wrapper = Wrapper::init(peripherals);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("running");

    loop {
        info!("Hello world!");

        wrapper.poll_keyboard();
        let color = Rgb565::RED;
        wrapper.display.clear(color).unwrap();
        let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        Text::new("Hello Rust!", Point::new(20, 30), style)
            .draw(&mut wrapper.display)
            .unwrap();

        info!("battery is {}", wrapper.read_battery_level());

        wrapper.poll_trackball();
        info!("moved {} {} {} {}", wrapper.trackball_right, wrapper.trackball_left, wrapper.trackball_up, wrapper.trackball_down);
        if let Ok(points) = wrapper.poll_touchscreen() {
            // stack allocated Vec containing 0-5 points
            info!("{:?}", points)
        }

        wrapper.delay.delay_millis(100);
    }
}
