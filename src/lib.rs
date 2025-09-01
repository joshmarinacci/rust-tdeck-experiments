#![no_std]

use esp_hal::spi::master::{Config as SpiConfig, Spi};
// use alloc::string::String;
use esp_hal::peripherals::{ADC1, GPIO4};
extern crate alloc;

use alloc::string::String;
use core::cell::RefCell;
use embedded_hal_bus::spi::{ExclusiveDevice, RefCellDevice};
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeManager};
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
            RefCellDevice<'static, Spi<'static, Blocking>, Output<'static>, Delay>,
            Output<'static>,
        >,
        ST7789,
        NoResetPin,
    >,
    i2c: I2c<'static, Blocking>,
    pub delay: Delay,
    adc: Adc<'static, ADC1<'static>, Blocking>,
    battery_pin: AdcPin<GPIO4<'static>, ADC1<'static>>,
    pub left:TrackballPin,
    pub right:TrackballPin,
    pub up:TrackballPin,
    pub down:TrackballPin,
    pub click:TrackballPin,
    pub touch:Gt911Blocking<I2c<'static, Blocking>>,
    pub volume_mgr: VolumeManager<SdCard<RefCellDevice<'static, Spi<'static, Blocking>,Output<'static>, Delay>,Delay>, DummyTimesource>,
}

pub struct TrackballPin {
    pin:Input<'static>,
    prev:bool,
    pub changed:bool,
}
impl TrackballPin {
    fn poll(&mut self) {
        self.changed = false;
        if self.pin.is_high() != self.prev {
            self.prev = self.pin.is_high();
            self.changed = true;
        }
    }
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
        self.left.poll();
        self.right.poll();
        self.up.poll();
        self.down.poll();
        self.click.poll();
    }

    pub fn poll_touchscreen(&mut self) -> Result<Vec<Point, 5>, Gt911Error<Error>> {
        self.touch.get_multi_touch(&mut self.i2c)
    }
}

static SPI_BUS:StaticCell<RefCell<Spi<Blocking>>> = StaticCell::new();


pub struct DummyTimesource();

impl TimeSource for DummyTimesource {
    // In theory you could use the RTC of the rp2040 here, if you had
    // any external time synchronizing device.
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 0,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
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
        // let spi_device = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
        let shared_spi_bus = RefCell::new(spi);
        let shared_spi_bus = SPI_BUS.init(shared_spi_bus);


        let tft_device = RefCellDevice::new(shared_spi_bus, tft_cs, spi_delay).expect("failed to create spi device");
        // let mut buffer = [0u8; 512];
        static DISPLAY_BUF: StaticCell<[u8; 512]> = StaticCell::new();
        let buffer = DISPLAY_BUF.init([0u8; 512]);
        let di = SpiInterface::new(tft_device, tft_dc, buffer);
        info!("building");
        let display = Builder::new(ST7789, di)
            .display_size(240, 320)
            .invert_colors(ColorInversion::Inverted)
            .color_order(ColorOrder::Rgb)
            .orientation(Orientation::new().rotate(Rotation::Deg90))
            .init(&mut delay)
            .unwrap();

        info!("initialized display");

        let BOARD_SDCARD_CS = peripherals.GPIO39;
        let sdmmc_cs = Output::new(BOARD_SDCARD_CS, High, OutputConfig::default());
        let sdcard_device = RefCellDevice::new(shared_spi_bus, sdmmc_cs, spi_delay).expect("failed to create spi device");
        let sdcard = SdCard::new(sdcard_device, delay);
        let mut volume_mgr = VolumeManager::new(sdcard, DummyTimesource {});

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
            volume_mgr,
            adc: Adc::new(peripherals.ADC1, adc_config),
            battery_pin: pin,
            left: TrackballPin {
                changed: false,
                prev: false,
                pin: Input::new(
                    peripherals.GPIO1,
                    InputConfig::default().with_pull(Pull::Up),
                )
            },
            right: TrackballPin {
                changed:false,
                prev:false,
                pin: Input::new(
                    peripherals.GPIO2,
                    InputConfig::default().with_pull(Pull::Up),
                )
            },
            up: TrackballPin {
                changed:false,
                prev:false,
                pin: Input::new(
                    peripherals.GPIO3,
                    InputConfig::default().with_pull(Pull::Up),
                )
            },
            down: TrackballPin {
                changed:false,
                prev:false,
                pin: Input::new(
                    peripherals.GPIO5,
                    InputConfig::default().with_pull(Pull::Up),
                )
            },
            click: TrackballPin {
                changed:false,
                prev:false,
                pin: Input::new(
                    peripherals.GPIO0,
                    InputConfig::default().with_pull(Pull::Up),
                )
            },
            // trackball_click_input: Input::new(
            //     peripherals.GPIO0,
            //     InputConfig::default().with_pull(Pull::Up),
            // ),
            // trackball_click:false,
            // trackball_up_input: Input::new(
            //     peripherals.GPIO3,
            //     InputConfig::default().with_pull(Pull::Up),
            // ),
            // trackball_up:false,
            // trackball_down_input: Input::new(
            //     peripherals.GPIO15,
            //     InputConfig::default().with_pull(Pull::Up),
            // ),
            // trackball_down:false,
        }
    }
}
