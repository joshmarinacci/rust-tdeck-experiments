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

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;


pub const LILYGO_KB_I2C_ADDRESS: u8 = 0x55;

struct Wrapper {
    display: Display<
        SpiInterface<'static,
            ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>,
            Output<'static>>,ST7789, NoResetPin>,
    i2c: I2c<'static, Blocking>,
    delay: Delay,
    adc:Adc<'static, ADC1<'static>, Blocking>,
    battery_pin:AdcPin<GPIO4<'static>, ADC1<'static>>,
}

impl Wrapper {
    pub(crate) fn poll_keyboard(&mut self) {
        let mut data = [0u8; 1];
        let kb_res = self.i2c.read(LILYGO_KB_I2C_ADDRESS, &mut data);
        match kb_res {
            Ok(_) => {
                if data[0] != 0x00 {
                    info!("kb_res = {:?}", String::from_utf8_lossy(&data));
                    self.delay.delay_millis(100);
                }
            }
            Err(e) => {
                info!("kb_res = {e}");
                self.delay.delay_millis(1000);
            }
        }
    }

    pub fn read_battery_level(&mut self) -> u16 {
        let pin_value: u16 = self.adc.read_blocking(&mut self.battery_pin);
        info!("bat adc is {pin_value} ");
        pin_value
    }
}

impl Wrapper {
    fn init(peripherals:Peripherals) -> Wrapper {
        let mut delay = Delay::new();

        // have to turn on the board and wait 500ms before using the keyboard
        let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
        board_power.set_high();
        delay.delay_millis(1000);

        let mut tft_cs = Output::new(peripherals.GPIO12, High, OutputConfig::default());
        tft_cs.set_high();
        let tft_miso = Input::new(peripherals.GPIO38, InputConfig::default().with_pull(Pull::Up));
        let tft_sck = peripherals.GPIO40;
        let tft_mosi = peripherals.GPIO41;
        let tft_dc = Output::new(peripherals.GPIO11, Low, OutputConfig::default());
        let mut tft_enable = Output::new(peripherals.GPIO42, High, OutputConfig::default());
        tft_enable.set_high();
        info!("creating spi device");
        let spi = Spi::new(
            peripherals.SPI2,
            SpiConfig::default().with_frequency(Rate::from_mhz(40)), // .with_mode(Mode::_0)
        )
            .unwrap()
            .with_sck(tft_sck)
            .with_miso(tft_miso)
            .with_mosi(tft_mosi);

        info!("setting up the display");
        let spi_delay = Delay::new();
        let spi_device = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
        // let mut buffer = [0u8; 512];
        static DISPLAY_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        let buffer = DISPLAY_BUF.init([0u8; 512]);
        let di = SpiInterface::new(spi_device, tft_dc, buffer);
        info!("building");
        let display = Builder::new(ST7789, di)
            .display_size(240, 320)
            .invert_colors(ColorInversion::Inverted)
            .color_order(ColorOrder::Rgb)
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut delay)
            .unwrap();

        info!("initialized display");

        // initialize keyboard
        let i2c = I2c::new(
            peripherals.I2C0,
            Config::default()
                .with_frequency(Rate::from_khz(100))
                .with_timeout(BusTimeout::Disabled),
        )
            .unwrap()
            .with_sda(peripherals.GPIO18)
            .with_scl(peripherals.GPIO8);

        // initialize battery monitor
        let analog_pin = peripherals.GPIO4;
        let mut adc_config:AdcConfig<ADC1> = AdcConfig::new();
        let mut pin:AdcPin<GPIO4, ADC1> = adc_config.enable_pin(analog_pin, Attenuation::_11dB);

        Wrapper {
            display,
            i2c,
            delay,
            adc:Adc::new(peripherals.ADC1, adc_config),
            battery_pin:pin,
        }
    }
}

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
        wrapper.delay.delay_millis(100);
    }
}
