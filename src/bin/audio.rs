#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::i2s::master::{DataFormat, I2s, Standard};
use esp_hal::time::Rate;
use esp_hal::{dma_buffers, main};
use log::info;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

const SINE: [i16; 64] = [
    0, 3211, 6392, 9511, 12539, 15446, 18204, 20787, 23169, 25329, 27244, 28897, 30272, 31356,
    32137, 32609, 32767, 32609, 32137, 31356, 30272, 28897, 27244, 25329, 23169, 20787, 18204,
    15446, 12539, 9511, 6392, 3211, 0, -3211, -6392, -9511, -12539, -15446, -18204, -20787, -23169,
    -25329, -27244, -28897, -30272, -31356, -32137, -32609, -32767, -32609, -32137, -31356, -30272,
    -28897, -27244, -25329, -23169, -20787, -18204, -15446, -12539, -9511, -6392, -3211,
];

const QUACK: &[u8] = include_bytes!("quack.wav");
// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    info!("Init!");
    esp_alloc::heap_allocator!(size: 72 * 1024);

    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    let delay = Delay::new();
    delay.delay_millis(1000);

    // let timg0 = TimerGroup::new(peripherals.TIMG0);
    // esp_hal_embassy::init(timg0.timer0);

    //     if #[cfg(any(feature = "esp32", feature = "esp32s2"))] {
    //         let dma_channel = peripherals.DMA_I2S0;
    //     } else {
    //         let dma_channel = peripherals.DMA_CH0;
    //     }
    // }

    let dma_channel = peripherals.DMA_CH0;
    // info!("peripherals {:?}",peripherals);

    let (_, _, tx_buffer, tx_descriptors) = dma_buffers!(0, 32000 * 2);

    let i2s = I2s::new(
        peripherals.I2S0,
        Standard::Philips,
        DataFormat::Data16Channel16,
        Rate::from_hz(44100),
        dma_channel,
    );
    // .into_async();

    // #define BOARD_I2S_WS        5
    // #define BOARD_I2S_BCK       7
    // #define BOARD_I2S_DOUT      6
    let mut i2s_tx = i2s
        .i2s_tx
        .with_bclk(peripherals.GPIO7)
        .with_ws(peripherals.GPIO5)
        .with_dout(peripherals.GPIO6)
        .build(tx_descriptors);

    let mut SAW: [i16; 256] = [0; 256];
    for i in 0..256 {
        SAW[i] = (i as i16) * 128;
    }

    let mut saw_buffer = [0i16; 1024];
    generate_sawtooth(&mut saw_buffer, 10_000); // amplitude up to +/- 10k

    // create unsafe data from the sine wave
    let data = unsafe { core::slice::from_raw_parts(&SAW as *const _ as *const u8, SAW.len() * 2) };

    // fill the buffer with the sine wave
    let buffer = tx_buffer;
    let mut idx = 0;
    for i in 0..buffer.len() {
        buffer[i] = QUACK[i % QUACK.len()];
    }
    // for i in 0..usize::max(data.len(), buffer.len()) {
    //     buffer[i] = data[idx];
    //
    //     idx += 1;
    //
    //     if idx >= data.len() {
    //         idx = 0;
    //     }
    // }

    let mut filler = [0u8; 10000];
    let mut idx = 32000 % data.len();

    info!("Start");
    let mut transaction = i2s_tx.write_dma_circular(buffer).unwrap();
    loop {
        for i in 0..filler.len() {
            filler[i] = data[(idx + i) % data.len()];
        }
        info!("Next");
        info!("can push {:?}", transaction.available());

        let written = transaction.push(&filler).unwrap();
        idx = (idx + written) % data.len();
        info!("written {}", written);
    }
}

fn generate_sawtooth(buffer: &mut [i16], amplitude: i16) {
    let len = buffer.len() as i16;
    for i in 0..buffer.len() {
        // Linearly ramp from -amplitude to +amplitude
        let value = ((i as i32 * 2 * amplitude as i32) / len as i32) - amplitude as i32;
        buffer[i] = value as i16;
    }
}
