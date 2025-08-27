#![no_std]

use esp_hal::spi::master::{Config as SpiConfig, Spi};
// use alloc::string::String;
use esp_hal::peripherals::{ADC1, GPIO4};
extern crate alloc;

use alloc::string::String;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::analog::adc::{Adc, AdcConfig, AdcPin, Attenuation};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::i2c::master::{BusTimeout, Config, Error, I2c};
use esp_hal::peripherals::Peripherals;
use esp_hal::time::Rate;
use esp_hal::Blocking;
use gt911::{Error as Gt911Error, Gt911, Gt911Blocking, Point};
use heapless::Vec;
use log::info;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use mipidsi::{Builder, Display, NoResetPin};
use static_cell::StaticCell;

const LILYGO_KB_I2C_ADDRESS: u8 = 0x55;

pub struct Wrapper {
    pub display: Display<
        SpiInterface<
            'static,
            ExclusiveDevice<Spi<'static, Blocking>, Output<'static>, Delay>,
            Output<'static>,
        >,
        ST7789,
        NoResetPin,
    >,
    i2c: I2c<'static, Blocking>,
    pub delay: Delay,
    adc: Adc<'static, ADC1<'static>, Blocking>,
    battery_pin: AdcPin<GPIO4<'static>, ADC1<'static>>,
    trackball_click_input: Input<'static>,
    trackball_right_input: Input<'static>,
    trackball_left_input: Input<'static>,
    trackball_up_input: Input<'static>,
    trackball_down_input: Input<'static>,
    pub trackball_click: bool,
    pub trackball_right: bool,
    pub trackball_left: bool,
    pub trackball_up: bool,
    pub trackball_down: bool,
    pub touch:Gt911Blocking<I2c<'static, Blocking>>,
}

impl Wrapper {
    pub fn poll_keyboard(&mut self) {
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

    pub fn poll_trackball(&mut self) {
        if self.trackball_click_input.is_high() != self.trackball_click {
            info!("trackball click changed ");
            self.trackball_click = self.trackball_click_input.is_high();
        }
        if self.trackball_right_input.is_high() != self.trackball_right {
            info!("trackball right changed ");
            self.trackball_right = self.trackball_right_input.is_high();
        }
        if self.trackball_left_input.is_high() != self.trackball_left {
            info!("trackball left changed ");
            self.trackball_left = self.trackball_left_input.is_high();
        }
        if self.trackball_up_input.is_high() != self.trackball_up {
            info!("trackball up changed ");
            self.trackball_up = self.trackball_up_input.is_high();
        }
        if self.trackball_down_input.is_high() != self.trackball_down {
            info!("trackball down changed ");
            self.trackball_down = self.trackball_down_input.is_high();
        }

    }

    pub fn poll_touchscreen(&mut self) -> Result<Vec<Point, 5>, Gt911Error<Error>> {
        self.touch.get_multi_touch(&mut self.i2c)
    }
}

impl Wrapper {
    pub fn init(peripherals: Peripherals) -> Wrapper {
        let mut delay = Delay::new();

        // have to turn on the board and wait 500ms before using the keyboard
        let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
        board_power.set_high();
        delay.delay_millis(1000);

        let mut tft_cs = Output::new(peripherals.GPIO12, High, OutputConfig::default());
        tft_cs.set_high();
        let tft_miso = Input::new(
            peripherals.GPIO38,
            InputConfig::default().with_pull(Pull::Up),
        );
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
        let mut i2c = I2c::new(
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
        let mut adc_config: AdcConfig<ADC1> = AdcConfig::new();
        let mut pin: AdcPin<GPIO4, ADC1> = adc_config.enable_pin(analog_pin, Attenuation::_11dB);


        let touch = Gt911Blocking::default();
        touch.init(&mut i2c).unwrap();

        // set up the trackball button pins
        Wrapper {
            display,
            i2c,
            delay,
            touch,
            adc: Adc::new(peripherals.ADC1, adc_config),
            battery_pin: pin,
            trackball_click_input: Input::new(
                peripherals.GPIO0,
                InputConfig::default().with_pull(Pull::Up),
            ),
            trackball_click:false,
            trackball_right_input: Input::new(
                peripherals.GPIO2,
                InputConfig::default().with_pull(Pull::Up),
            ),
            trackball_right:false,
            trackball_left_input: Input::new(
                peripherals.GPIO1,
                InputConfig::default().with_pull(Pull::Up),
            ),
            trackball_left:false,
            trackball_up_input: Input::new(
                peripherals.GPIO3,
                InputConfig::default().with_pull(Pull::Up),
            ),
            trackball_up:false,
            trackball_down_input: Input::new(
                peripherals.GPIO15,
                InputConfig::default().with_pull(Pull::Up),
            ),
            trackball_down:false,
        }
    }
}
