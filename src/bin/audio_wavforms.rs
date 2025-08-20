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
extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use core::error::Error;
use core::f32::consts::TAU;
use core::time::Duration;
use embassy_executor::Spawner;
// use esp_backtrace as _;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::{
    dma_buffers,
    i2s::master::{DataFormat, I2s, Standard},
    time::Rate,
    timer::timg::TimerGroup,
};

use log::{error, info};
use micromath::F32Ext;

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

const TIMEOUT: Duration = Duration::from_millis(100);
const SAMPLE_RATE_HZ: u32 = 44100;

fn make_sawtooth(freq: f32, vol: f32) -> Vec<u8> {
    let buffer_size = (SAMPLE_RATE_HZ as f32 / freq) as usize;
    let mut buffer = vec![0; buffer_size * 4];
    let mut value: f32 = 0.0;
    let mut value_inc = 0.1 / (buffer_size as f32);

    for i in (0..buffer.len()).step_by(4) {
        let i_value = (value * vol * (i16::MAX as f32)) as i16 as u16;

        buffer[i] = (i_value & 0x00ff) as u8;
        buffer[i + 1] = ((i_value & 0xff00) >> 8) as u8;
        buffer[i + 2] = (i_value & 0x00ff) as u8;
        buffer[i + 3] = ((i_value & 0xff00) >> 8) as u8;
        value += value_inc;

        if value_inc > 0.0 && value > 1.0 {
            value = 2.0 - value;
            value_inc = -value_inc;
        } else if value_inc < 0.0 && value < 1.0 {
            value = -2.0 - value;
            value_inc = -value_inc;
        }
    }

    buffer
}

fn make_sawtooth_sample(freq: f32, vol: f32, i: usize) -> u16 {
    let buffer_size = (SAMPLE_RATE_HZ as f32 / freq) as usize;
    let mut value: f32 = 0.0;
    let mut value_inc = 0.1 / (buffer_size as f32);
    let value = value_inc * (i as f32);
    let i_value = (value * vol * (i16::MAX as f32)) as i16 as u16;
    return i_value;
}

struct SampleSource {
    i: u8,
}

impl SampleSource {
    // choose values which DON'T restart on every descriptor buffer's start
    const ADD: u8 = 5;
    const CUT_OFF: u8 = 113;

    fn new() -> Self {
        Self { i: 0 }
    }
}

impl Iterator for SampleSource {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.i;
        self.i = (i + Self::ADD) % Self::CUT_OFF;
        Some(i)
    }
}


#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    info!("Start");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let mut decoder = nanomp3::Decoder::new();

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
    )
        .into_async();

    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(peripherals.GPIO7)
        .with_ws(peripherals.GPIO5)
        .with_dout(peripherals.GPIO6)
        .build(tx_descriptors);

    let data =
        unsafe { core::slice::from_raw_parts(&SINE as *const _ as *const u8, SINE.len() * 2) };
    // let data = make_sawtooth(240.0, 0.2);

    let buffer = tx_buffer;
    // let mut idx = 0;
    // for i in 0..usize::min(data.len(), buffer.len()) {
    //     buffer[i] = data[idx];
    //
    //     idx += 1;
    //
    //     if idx >= data.len() {
    //         idx = 0;
    //     }
    // }

    // let mut filler = [0u8; 10000];
    // let mut idx = 32000 % data.len();

    info!("Start");
    let mut transaction = i2s_tx.write_dma_circular_async(buffer).unwrap();
    let freq = 120.0 * 2.0;
    const OMEGA_INC: f32 = TAU / SAMPLE_RATE_HZ as f32;
    // let mut omega: f32 = 0.0;
    let mut count = 0;
    // let mut vol: f32 = 0.1;
    let mut samples = SampleSource::new();
    loop {
        // for i in (0..filler.len()).step_by(2) {
        //     filler[i] = data[(idx + i) % data.len()];
        //     filler[i + 1] = data[(idx + i + 1) % data.len()];
        // let sample = ((omega * freq).sin() * vol * (i16::MAX as f32)) as u16;
        // filler[i] = (sample & 0x00ff) as u8;
        // filler[i + 1] = ((sample & 0xff00) >> 8) as u8;
        // omega += OMEGA_INC;
        // if omega >= TAU {
        //     omega -= TAU;
        // }
        // }
        // info!("Next");

        // let written = transaction.push(&filler).await.unwrap();
        let written = transaction.push_with(|buf| {
            for b in buf.iter_mut() {
                *b = samples.next().unwrap();
            }
            buf.len()
        }).await.unwrap();
        // idx = (idx + written) % data.len();
        info!("written {}", written);
        count += 1;
        if count >= 20 {
            break;
        }
    }
}
