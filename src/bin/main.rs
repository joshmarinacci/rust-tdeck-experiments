#![no_std]
#![no_main]

use esp_hal::clock::CpuClock;
use esp_hal::gpio::{GpioPin, Input, InputConfig, Pull};
use esp_hal::main;
use esp_hal::time::{Duration, Instant};
use esp_hal::timer::timg::TimerGroup;
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

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let _init = esp_wifi::init(
        timg0.timer0,
        esp_hal::rng::Rng::new(peripherals.RNG),
        peripherals.RADIO_CLK,
    )
    .unwrap();
    

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
    let button = Input::new(peripherals.GPIO0,InputConfig::default().with_pull(Pull::Up));
    testTrackballButtonForever(button);
    

    // loop_helloworld_forever();
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
