#![no_std]
#![no_main]
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{GpioPin, Input, InputConfig, Io, Output, OutputConfig, Pull};
use esp_hal::analog::adc;
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::delay::Delay;
use esp_hal::gpio::DriveMode::PushPull;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use esp_hal::main;
use esp_hal::spi::{ master::{Spi, Config as SpiConfig } };
use esp_hal::spi::Mode;
use esp_hal::time::{Duration, Instant, Rate};
use esp_hal::timer::timg::TimerGroup;
use log::info;
use embedded_hal_bus::spi::ExclusiveDevice;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
    mono_font::{ ascii::FONT_6X10, MonoTextStyle}
};
use embedded_graphics::framebuffer::buffer_size;
use mipidsi::{models::ST7789, Builder};
use mipidsi::interface::SpiInterface;
use mipidsi::options::{ColorInversion, ColorOrder};


#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

#[main]
fn main() -> ! {
    // generator version: 0.3.1

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("running");

    loop {
        info!("Hello world!");
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}