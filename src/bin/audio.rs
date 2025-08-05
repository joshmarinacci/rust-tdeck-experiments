#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use esp_hal::clock::CpuClock;
use esp_hal::{dma_buffers, main, peripherals};
use esp_hal::delay::Delay;
use esp_hal::i2s::master::{DataFormat, I2s};
use esp_hal::i2s::master::Standard::Philips;
use esp_hal::time::{Duration, Instant, Rate};
use log::info;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

// #[main]
// fn main() -> ! {
//     esp_println::logger::init_logger_from_env();
//     let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
//     esp_hal::init(config);
//
//     esp_alloc::heap_allocator!(size: 72 * 1024);
//
//     info!("running");
//
//     loop {
//         info!("Hello world!");
//         let delay_start = Instant::now();
//         while delay_start.elapsed() < Duration::from_millis(500) {}
//     }
// }


const DATA_SIZE: usize = 1024 * 10;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let delay = Delay::new();

    // let system = peripherals.SYSTEM.split();
    // let clocks = ClockControl::boot_defaults(system.clock_control).freeze();

    // let io = peripherals.GPIO.split();

    let bclk = peripherals.GPIO7;// .io.pins.gpio7;
    let lrclk = peripherals.GPIO5;
    // let lrclk = io.pins.gpio5;
    // let dout = io.pins.gpio6;
    let dout = peripherals.GPIO6;
    // let dma = peripherals.DMA1;

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(DATA_SIZE);

    // let dma = peripherals.D;
    // I2s::new(peripherals.I2S0, bclk, lrclk, dout)
    // let mut i2s = I2s::new(
    //     peripherals.I2S0,
    //     esp_hal::i2s::master::Standard::Philips,
    //     DataFormat::Data16Channel16,
    //     Rate::from_hz(44100u32),
    // );

    //     bclk,
    //     lrclk,
    //     Some(dout),
    //     None,
    //     &clocks,
    // )
    //     .unwrap();

    // 16-bit mono sample (e.g., 440Hz sine wave or square wave)
    let sample: i16 = 3000;

    loop {
    //     let buf = [sample; 64]; // 64 samples per write
    //     i2s.write(&buf).unwrap();

        delay.delay_millis(100);
    }
}