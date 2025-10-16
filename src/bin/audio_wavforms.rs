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
    i2s::master::{DataFormat, I2s},
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_hal::i2s::master::Config;
use esp_rtos::main;
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
struct SineWaveSource {
    i: i32,
}

impl SineWaveSource {
    fn new() -> Self {
        Self { i: 0 }
    }
}

impl Iterator for SineWaveSource {
    type Item = f32;
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.i;
        self.i += 1;
        let samp: f32 = ((i as f32) / 10.0 * 0.8).sin() * (i16::MAX as f32) * 0.5;
        Some(samp)
    }
}

struct SawtoothWaveSource {
    phase: u32,     // 0..=u32::MAX wraps around (fixed-point phase)
    step: u32,      // how much to advance per sample
    amplitude: i16, // max amplitude
}

impl SawtoothWaveSource {
    pub fn new(freq: u32, sample_rate: u32, amplitude: i16) -> Self {
        // step = freq / sample_rate, in Q32 fixed point
        let step = ((freq as u64) << 32) / (sample_rate as u64);
        Self {
            phase: 0,
            step: step as u32,
            amplitude,
        }
    }
}

impl Iterator for SawtoothWaveSource {
    type Item = i16;
    fn next(&mut self) -> Option<i16> {
        let frac = self.phase >> 16; // use upper 16 bits as position (0..65535)
                                     // Map 0..65535 → -amplitude..+amplitude
        let val = (frac as i32 * (self.amplitude as i32 * 2) / 65536) - self.amplitude as i32;

        self.phase = self.phase.wrapping_add(self.step);
        Some(val as i16)
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    info!("Start");
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timer_g1 = TimerGroup::new(peripherals.TIMG1);
    esp_rtos::start(timer_g1.timer0);
    esp_alloc::heap_allocator!(size: 72 * 1024);
    info!("heap is {}", esp_alloc::HEAP.stats());
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    let delay = Delay::new();
    delay.delay_millis(1000);

    let (_, _, tx_buffer, tx_descriptors) = dma_buffers!(0, 32000);

    let i2s = I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        Config::new_tdm_philips()
            .with_data_format(DataFormat::Data16Channel16)
            .with_sample_rate(Rate::from_hz(44100))
    ).unwrap().into_async();

    let i2s_tx = i2s
        .i2s_tx
        .with_bclk(peripherals.GPIO7)
        .with_ws(peripherals.GPIO5)
        .with_dout(peripherals.GPIO6)
        .build(tx_descriptors);

    let buffer = tx_buffer;
    let mut transaction = i2s_tx.write_dma_circular_async(buffer).unwrap();
    let mut count = 0;
    // let mut samples = SineWaveSource::new();
    let mut samples = SawtoothWaveSource::new(262, 44_100, i16::MAX / 4); // 440Hz, half volume
    loop {
        let written = transaction
            .push_with(|buf| {
                for i in (0..buf.len()).step_by(4) {
                    let samp = samples.next().unwrap();
                    let isamp = samp as u16;
                    buf[i + 0] = (isamp & 0x00ff) as u8;
                    buf[i + 1] = ((isamp & 0xff00) >> 8) as u8;
                    buf[i + 2] = (isamp & 0x00ff) as u8;
                    buf[i + 3] = ((isamp & 0xff00) >> 8) as u8;
                }
                buf.len()
            })
            .await
            .unwrap();
        info!("written {}", written);
        count += 1;
        if count >= 50 {
            break;
        }
    }
}
