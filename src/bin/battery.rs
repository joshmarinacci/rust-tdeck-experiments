#![no_std]
#![no_main]
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::analog::adc::{Adc, AdcConfig, Attenuation};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High};
use esp_hal::main;
use log::info;


#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

extern crate alloc;

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let delay = Delay::new();

    // turn on the board power
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    info!("running");

    let analog_pin = peripherals.GPIO4;
    let mut adc_config = AdcConfig::new();
    let mut pin = adc_config.enable_pin(analog_pin, Attenuation::_11dB);
    let mut adc1 = Adc::new(peripherals.ADC1, adc_config);

    loop {
        info!("getting the pin value");
        let pin_value: u16 = adc1.read_blocking(&mut pin);
        info!("bat adc is {} ", pin_value);
        delay.delay_millis(1500);
    }

}