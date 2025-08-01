#![no_std]
#![no_main]

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::main;
use esp_hal::spi::{ master::{Spi, Config as SpiConfig } };
use esp_hal::time::{Duration, Instant, Rate};
use log::info;
use embedded_hal_bus::spi::ExclusiveDevice;


use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
    mono_font::{ ascii::FONT_6X10, MonoTextStyle}
};
use mipidsi::{models::ST7789, Builder};
use mipidsi::interface::SpiInterface;
use mipidsi::options::{ColorInversion, ColorOrder};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

pub const LILYGO_KB_I2C_ADDRESS: u8 =     0x55;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let mut delay = Delay::new();

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    // set TFT CS to high
    let mut tft_cs = Output::new(peripherals.GPIO12, High, OutputConfig::default());
    tft_cs.set_high();
    let tft_miso = Input::new(peripherals.GPIO38, InputConfig::default().with_pull(Pull::Up));
    let tft_sck = peripherals.GPIO40;
    let tft_mosi = peripherals.GPIO41;
    let tft_dc = Output::new(peripherals.GPIO11, Low, OutputConfig::default());
    let mut tft_enable = Output::new(peripherals.GPIO42, High, OutputConfig::default());
    tft_enable.set_high();

    info!("creating spi device");
    let spi = Spi::new(peripherals.SPI2, SpiConfig::default()
        .with_frequency(Rate::from_mhz(40))
        // .with_mode(Mode::_0)
    ).unwrap()
        .with_sck(tft_sck)
        .with_miso(tft_miso)
        .with_mosi(tft_mosi)
    ;
    let mut buffer = [0u8; 512];

    info!("setting up the display");
    let spi_delay = Delay::new();
    let spi_device = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
    let di = SpiInterface::new(spi_device, tft_dc, &mut buffer);
    info!("building");
    let mut display = Builder::new(ST7789,di)
        // .reset_pin(tft_enable)
        .display_size(240,320)
        .invert_colors(ColorInversion::Inverted)
        .color_order(ColorOrder::Rgb)
        // .display_size(320,240)
        .init(&mut delay).unwrap();

    info!("initialized");
    // wait for everything to boot up
    // delay.delay_millis(500);
    let colors = [Rgb565::BLACK, Rgb565::WHITE, Rgb565::RED, Rgb565::GREEN, Rgb565::BLUE];
    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
    for _ in 1..10 {
        for color in colors.iter() {
            display.clear(*color).unwrap();
            Text::new("Hello Rust!", Point::new(20, 30), style).draw(&mut display).unwrap();
            info!("color {:?}", *color);
            delay.delay_millis(3000);
        }
    }

    info!("Display initialized");

    loop_helloworld_forever();
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-beta.0/examples/src/bin
}

fn loop_helloworld_forever() -> ! {
    loop {
        info!("Hello world!");
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}
