#![no_std]
#![no_main]

use alloc::string::String;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High};
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use esp_hal::main;
use esp_hal::time::{Rate};
use log::info;


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
    let delay = Delay::new();
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


    let mut i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)).with_timeout(BusTimeout::Disabled),
    )
        .unwrap()
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO8);

    
    // info!("turning on the keyboard backlight");
    // let mut buf = [0u8; 2];
    // buf[0] = 0x02;

    // for val in 0..255 {
    //     buf[1] = val;
    //     let mut resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
    //     info!("response {:?}",resp);
    //     delay.delay_millis(10);
    // }

    // buf[1] = 0x99;
    // resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
    // info!("response {:?}",resp);
    // delay.delay_millis(500);
    //
    // buf[1] = 0xFF;
    // resp = i2c.write(LILYGO_KB_I2C_ADDRESS,&buf);
    // info!("response {:?}",resp);
    // delay.delay_millis(500);


    info!("looping over the keyboard");
    loop {
        let mut data = [0u8; 1];
        let kb_res = i2c.read(LILYGO_KB_I2C_ADDRESS, &mut data);
        match kb_res {
            Ok(_) => {
                if data[0] != 0x00 {
                    info!("kb_res = {:?}", String::from_utf8_lossy(&data));
                }
            },
            Err(e) => {
                info!("kb_res = {}", e);
                delay.delay_millis(1000);
            }
        }
    }

}

