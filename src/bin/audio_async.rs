//! This shows how to transmit data continuously via I2S.
//!
//! Without an additional I2S sink device you can inspect the BCLK, WS
//! and DOUT with a logic analyzer.
//!
//! You can also connect e.g. a PCM510x to hear an annoying loud sine tone (full
//! scale), so turn down the volume before running this example.
//!
//! The following wiring is assumed:
//! - BCLK => GPIO2
//! - WS   => GPIO4
//! - DOUT => GPIO5
//!
//! PCM510x:
//! | Pin   | Connected to    |
//! |-------|-----------------|
//! | BCK   | GPIO1           |
//! | DIN   | GPIO3           |
//! | LRCK  | GPIO2           |
//! | SCK   | Gnd             |
//! | GND   | Gnd             |
//! | VIN   | +3V3            |
//! | FLT   | Gnd             |
//! | FMT   | Gnd             |
//! | DEMP  | Gnd             |
//! | XSMT  | +3V3            |

//% CHIPS: esp32 esp32c3 esp32c6 esp32h2 esp32s2 esp32s3

#![no_std]
#![no_main]

use embassy_executor::Spawner;
// use esp_backtrace as _;
use esp_hal::{
    dma_buffers,
    i2s::master::{DataFormat, I2s, Standard},
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Output, OutputConfig};

use log::{error, info};

#[panic_handler]
fn panic(nfo: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {:?}", nfo);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

const SINE: [i16; 64] = [
    0, 3211, 6392, 9511, 12539, 15446, 18204, 20787, 23169, 25329, 27244, 28897, 30272, 31356,
    32137, 32609, 32767, 32609, 32137, 31356, 30272, 28897, 27244, 25329, 23169, 20787, 18204,
    15446, 12539, 9511, 6392, 3211, 0, -3211, -6392, -9511, -12539, -15446, -18204, -20787, -23169,
    -25329, -27244, -28897, -30272, -31356, -32137, -32609, -32767, -32609, -32137, -31356, -30272,
    -28897, -27244, -25329, -23169, -20787, -18204, -15446, -12539, -9511, -6392, -3211,
];

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    info!("Start");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    info!("init-ting embassy");
    let timer_g1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer_g1.timer0);

    esp_alloc::heap_allocator!(size: 72 * 1024);
    info!("heap is {}", esp_alloc::HEAP.stats());


    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    let delay = Delay::new();
    delay.delay_millis(1000);

    let dma_channel = peripherals.DMA_CH0;
    let (_, _, tx_buffer, tx_descriptors) = dma_buffers!(0, 32000);

    let i2s = I2s::new(
        peripherals.I2S0,
        Standard::Philips,
        DataFormat::Data16Channel16,
        Rate::from_hz(44100),
        dma_channel,
    ).into_async();

    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(peripherals.GPIO7)
        .with_ws(peripherals.GPIO5)
        .with_dout(peripherals.GPIO6)
        .build(tx_descriptors);

    let data =
        unsafe { core::slice::from_raw_parts(&SINE as *const _ as *const u8, SINE.len() * 2) };

    let buffer = tx_buffer;
    let mut idx = 0;
    for i in 0..usize::min(data.len(), buffer.len()) {
        buffer[i] = data[idx];

        idx += 1;

        if idx >= data.len() {
            idx = 0;
        }
    }

    let mut filler = [0u8; 10000];
    let mut idx = 32000 % data.len();

    info!("Start");
    let mut transaction = i2s_tx.write_dma_circular_async(buffer).unwrap();
    loop {
        for i in 0..filler.len() {
            filler[i] = data[(idx + i) % data.len()];
        }
        info!("Next");

        let written = transaction.push(&filler).await.unwrap();
        idx = (idx + written) % data.len();
        info!("written {}", written);
    }
}