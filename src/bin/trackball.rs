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
    // standard setup
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 72 * 1024);
    let mut delay = Delay::new();

    // turn on the board
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    
    // setup the trackball button pin
    let tdeck_track_click = Input::new(peripherals.GPIO0, InputConfig::default().with_pull(Pull::Up));// .pins.gpio0.into_pull_up_input();
    // connect to the left and right trackball pins
    let tdeck_trackball_right = Input::new(peripherals.GPIO15, InputConfig::default().with_pull(Pull::Down));//.pins.gpio3.into_pull_up_input(); // G01  GS1
    let tdeck_trackball_left = Input::new(peripherals.GPIO1, InputConfig::default().with_pull(Pull::Down));//.pins.gpio3.into_pull_up_input(); // G01  GS1
    let mut last_right_high = false;
    let mut last_left_high = false;

    info!("running");
    loop {
        info!("button pressed is {} ", tdeck_track_click.is_low());
        if(tdeck_trackball_right.is_high() != last_right_high) {
            info!("trackball right changed ");
            last_right_high = tdeck_trackball_right.is_high();
        }
        if(tdeck_trackball_left.is_high() != last_left_high) {
            info!("trackball left changed ");
            last_left_high = tdeck_trackball_left.is_high();
        }
        // wait one msec
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(1) {}
    }

}