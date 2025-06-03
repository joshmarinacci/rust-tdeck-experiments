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

pub const LILYGO_KB_I2C_ADDRESS: u8 =     0x55;

#[main]
fn main() -> ! {
    // generator version: 0.3.1

    esp_println::logger::init_logger_from_env();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    // let timg0 = TimerGroup::new(peripherals.TIMG0);
    // let _init = esp_wifi::init(
    //     timg0.timer0,
    //     esp_hal::rng::Rng::new(peripherals.RNG),
    //     peripherals.RADIO_CLK,
    // )
    // .unwrap();


    // let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    // let i2c0_raw = I2C::new(
    //     peripherals.I2C0,
    //     io.pins.gpio18,
    //     io.pins.gpio8,
    //     100u32.kHz(),
    //     &mut system.peripheral_clock_control,
    //     &clocks,
    // );
    // let i2c0_bus: &'static _ = shared_bus::new_xtensa!(I2c0RawBusType = i2c0_raw).unwrap();
    // let i2c0_proxy = i2c0_bus.acquire_i2c();


    // trackball click
    // let tdeck_track_click:GpioPin<Input<Pull>,0> = io.pins.gpio0.into_pull_up_input();
    // let button = Input::new(peripherals.GPIO0,InputConfig::default().with_pull(Pull::Up));
    // testTrackballButtonForever(button);
    



    // === read the battery value
    // let analog_pin = peripherals.GPIO4;
    // let mut adc_config = AdcConfig::new();
    // let mut pin = adc_config.enable_pin(analog_pin, Attenuation::_11dB);
    // let mut adc1 = Adc::new(peripherals.ADC2, adc_config);
    let mut delay = Delay::new();
    //
    // loop {
    //     info!("getting the pin value");
    //     // let pin_value = bat_adc.read_oneshot(&mut pin);
    //     // let pin_value: u16 = nb::block!(adc1.read_oneshot(&mut pin)).unwrap();
    //     let pin_value: u16 = adc1.read_oneshot(&mut pin).unwrap();
    //     info!("bat adc is {} ", pin_value);
    //     delay.delay_millis(1500);
    // }

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);


    // ==== display setup ====
    // https://github.com/Xinyuan-LilyGO/T-Deck/blob/master/examples/HelloWorld/HelloWorld.ino

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
    let mut spi = Spi::new(peripherals.SPI2, SpiConfig::default()
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

    info!("initted");
    // wait for everything to boot up
    // delay.delay_millis(500);
    for n in 1..10 {
        display.clear(Rgb565::BLACK).unwrap();
        info!("black");
        delay.delay_millis(1000);

        display.clear(Rgb565::WHITE).unwrap();
        info!("white");
        delay.delay_millis(1000);

        display.clear(Rgb565::RED).unwrap();
        info!("red");
        delay.delay_millis(1000);

        display.clear(Rgb565::GREEN).unwrap();
        info!("green");
        delay.delay_millis(1000);

        display.clear(Rgb565::BLUE).unwrap();
        info!("blue");
        delay.delay_millis(1000);
    }

    info!("Display initialized");

    /*
    let mut i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)).with_timeout(BusTimeout::Disabled),
    )
        .unwrap()
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO8);


    info!("turning on the keyboard backlight");
    let mut buf = [0u8; 2];
    buf[0] = 0x02;

    for val in 0..255 {
        buf[1] = val;
        let mut resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
        info!("response {:?}",resp);
        delay.delay_millis(10);
    }

    // buf[1] = 0x99;
    // resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
    // info!("response {:?}",resp);
    // delay.delay_millis(500);
    //
    // buf[1] = 0xFF;
    // resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
    // info!("response {:?}",resp);
    // delay.delay_millis(500);


    let tdeck_trackball_right = Input::new(peripherals.GPIO15, InputConfig::default().with_pull(Pull::Down));//.pins.gpio3.into_pull_up_input(); // G01  GS1
    let tdeck_trackball_left = Input::new(peripherals.GPIO1, InputConfig::default().with_pull(Pull::Down));//.pins.gpio3.into_pull_up_input(); // G01  GS1
    let mut last_right_high = false;
    let mut last_left_high = false;
    loop {
        if(tdeck_trackball_right.is_high() != last_right_high) {
            info!("trackball right changed ");
            last_right_high = tdeck_trackball_right.is_high();
        }
        if(tdeck_trackball_left.is_high() != last_left_high) {
            info!("trackball left changed ");
            last_left_high = tdeck_trackball_left.is_high();
        }
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(1) {}
    }

    info!("looping over the keyboard");
    loop {
        let mut data = [0u8; 1];
        let kb_res = i2c.read(LILYGO_KB_I2C_ADDRESS, &mut data);
        match kb_res {
            Ok(kb_res) => {
                if(data[0] != 0x00) {
                    info!("kb_res = {:?}", data);
                }
            },
            Err(e) => {
                info!("kb_res = {}", e);
                delay.delay_millis(1000);
            }
        }
    }

*/

    loop_helloworld_forever();
    // for inspiration have a look at the examples at https://github.com/esp-rs/esp-hal/tree/esp-hal-v1.0.0-beta.0/examples/src/bin
}

fn testTrackballButtonForever(button: Input) -> ! {
    loop {
        info!("button pressed is {} ", button.is_low());
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}

fn loop_helloworld_forever() -> ! {
    loop {
        info!("Hello world!");
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}
