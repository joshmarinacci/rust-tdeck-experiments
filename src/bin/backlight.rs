#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::gpio::{DriveMode, Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::main;
use esp_hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::spi::master::Spi;
use esp_hal::time::Rate;
use log::info;
use mipidsi::Builder;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use esp_hal::spi::master::{Config as SpiConfig};

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("starting");

    let mut delay = Delay::new();

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);
    info!("the board is on now");



    // standard display setup
    let mut TFT_CS = Output::new(peripherals.GPIO12, High, OutputConfig::default());
    TFT_CS.set_high();
    let tft_dc = Output::new(peripherals.GPIO11, Low, OutputConfig::default());
    // let mut tft_enable = Output::new(peripherals.GPIO42, High, OutputConfig::default());
    // tft_enable.set_high();

    let spi = Spi::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)),
    )
        .unwrap()
        .with_sck(peripherals.GPIO40)
        .with_miso(Input::new(
            peripherals.GPIO38,
            InputConfig::default().with_pull(Pull::Up),
        ))
        .with_mosi(peripherals.GPIO41);

    let mut buffer = [0u8; 512];

    info!("setting up the display");
    let spi_delay = Delay::new();
    let spi_device = ExclusiveDevice::new(spi, TFT_CS, spi_delay).unwrap();
    let di = SpiInterface::new(spi_device, tft_dc, &mut buffer);
    info!("building");
    let mut display = Builder::new(ST7789, di)
        .display_size(240, 320)
        .invert_colors(ColorInversion::Inverted)
        .color_order(ColorOrder::Rgb)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut delay)
        .unwrap();

    // fill display with white
    display.clear(*&Rgb565::WHITE).unwrap();


    // write text in black
    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK);
    Text::new("Cycle backlight from 0 to 100% over a second.", Point::new(20, 30), style)
        .draw(&mut display)
        .unwrap();


    // setup LEDC as a PWM to control the backlight
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let mut lstimer0 = ledc.timer::<LowSpeed>(timer::Number::Timer0);
    lstimer0.configure(timer::config::Config {
        duty: timer::config::Duty::Duty5Bit,
        clock_source: timer::LSClockSource::APBClk,
        frequency: Rate::from_khz(24),
    }).unwrap();
    let mut channel0 = ledc.channel(channel::Number::Channel0, peripherals.GPIO42);
    channel0
        .configure(channel::config::Config {
            timer: &lstimer0,
            duty_pct: 0,
            drive_mode: DriveMode::PushPull,
        }).unwrap();


    // fade the backlight from 0% to 100% over one second, then back again.
    loop {
        // You could animate the brightness here if you like
        channel0.start_duty_fade(0, 100, 1000).unwrap();
        while channel0.is_duty_fade_running() {}
        channel0.start_duty_fade(100, 0, 1000).unwrap();
        while channel0.is_duty_fade_running() {}
    }
}