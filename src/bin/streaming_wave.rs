#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_sdmmc::{SdCard, Timestamp, VolumeIdx, VolumeManager};
// use critical_section as cs;
// use esp_hal::{
    // clock::ClockControl,
    // dma::{Dma, DmaPriority},
    // entry,
    // gpio::Io,
    // i2s::{Channel as I2sChannel, DataFormat as I2sDataFmt, I2s, I2sTx, PinsBclkWsDout, Standard as I2sStd},
    // peripherals::Peripherals,
    // prelude::*,
    // spi::{Spi, SpiMode},
    // timer::TimerGroup,
// };
use esp_hal::{main};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Output, OutputConfig};
use esp_hal::i2s::master::Standard;
use esp_hal::time::Rate;
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use heapless::spsc::Queue;
// use panic_halt as _;
use log::{error, info};

#[panic_handler]
fn panic(nfo: &core::panic::PanicInfo) -> ! {
    error!("PANIC: {:?}", nfo);
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();
extern crate alloc;

// ---------- Config ----------
const CHUNK_BYTES: usize = 8192; // increase if you get underruns
const WAV_HEADER_LEN: usize = 44;

// T-Deck pins
const SD_SCK: i32 = 39;
const SD_MOSI: i32 = 40;
const SD_MISO: i32 = 38;
const SD_CS: i32 = 41;

const I2S_BCLK: i32 = 42;
const I2S_WS: i32 = 1;
const I2S_DOUT: i32 = 2;

// ---------- DMA buffers ----------
#[repr(align(4))]
struct Aligned<T>(T);
static mut DMA_BUF_A: Aligned<[u32; CHUNK_BYTES / 4]> = Aligned([0; CHUNK_BYTES / 4]);
static mut DMA_BUF_B: Aligned<[u32; CHUNK_BYTES / 4]> = Aligned([0; CHUNK_BYTES / 4]);

// Queue to track which buffer is currently playing (0 or 1)
static mut Q_STORAGE: MaybeUninit<Queue<u8, 4>> = MaybeUninit::uninit();

// ---------- WAV info ----------
#[derive(Clone, Copy)]
struct WavInfo {
    audio_format: u16,   // 1 = PCM
    num_channels: u16,   // 1 or 2
    sample_rate: u32,
    bits_per_sample: u16,// 16
    data_size: u32,      // bytes
    block_align: u16,    // bytes per frame (channels * bits/8)
}

// Minimal parser for 16-bit PCM, finds the "fmt " & "data" chunks in the 44B header
fn parse_wav_header(h: &[u8]) -> Option<WavInfo> {
    use byteorder::{ByteOrder, LittleEndian as LE};
    if h.len() < 44 || &h[0..4] != b"RIFF" || &h[8..12] != b"WAVE" { return None; }

    let mut p = 12usize;
    let mut fmt_found = false;
    let mut audio_format = 0u16;
    let mut num_channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut block_align = 0u16;
    let mut data_size = 0u32;

    while p + 8 <= h.len() {
        let id = &h[p..p+4];
        let sz = LE::read_u32(&h[p+4..p+8]) as usize;
        let body = p + 8;
        let next = body + sz;

        if id == b"fmt " && body + 16 <= h.len() {
            audio_format     = LE::read_u16(&h[body..body+2]);
            num_channels     = LE::read_u16(&h[body+2..body+4]);
            sample_rate      = LE::read_u32(&h[body+4..body+8]);
            let _byte_rate   = LE::read_u32(&h[body+8..body+12]);
            block_align      = LE::read_u16(&h[body+12..body+14]);
            bits_per_sample  = LE::read_u16(&h[body+14..body+16]);
            fmt_found = true;
        } else if id == b"data" {
            data_size = (sz as u32);
            break;
        }
        // chunks are word-aligned
        p = next + (next & 1);
    }

    if !fmt_found || data_size == 0 { return None; }
    Some(WavInfo {
        audio_format, num_channels, sample_rate, bits_per_sample, data_size, block_align
    })
}

// Read PCM bytes from SD and pack to u32 I2S frames (L: high 16, R: low 16)
// fn fill_frames_from_sd<DEV: embedded_sdmmc::BlockDevice>(
//     ctrl: &mut embedded_sdmmc::Controller<DEV, DummyTime>,
//     vol: &mut embedded_sdmmc::Volume,
//     file: &mut embedded_sdmmc::File,
//     out_frames: &mut [u32],
//     w: &WavInfo,
// ) -> usize {
//     // Temp byte buffer: size in bytes equals out_frames*4 (16b stereo = 4 bytes per frame)
//     let mut tmp = [0u8; CHUNK_BYTES];
//
//     let bytes_per_sample = (w.bits_per_sample / 8) as usize; // 2
//     let ch = w.num_channels as usize; // 1 or 2
//
//     // Compute how many PCM bytes we'd like to fill
//     let want_frames = out_frames.len();
//     let want_pcm_bytes = want_frames * bytes_per_sample * ch;
//     let to_read = core::cmp::min(tmp.len(), want_pcm_bytes);
//
//     let n = ctrl.read(vol, file, &mut tmp[..to_read]).unwrap_or(0);
//     if n == 0 { return 0; }
//
//     let mut produced = 0usize;
//     let mut i = 0usize;
//     while i + bytes_per_sample * ch <= n && produced < out_frames.len() {
//         // 16-bit little-endian
//         let l = i16::from_le_bytes([tmp[i], tmp[i+1]]) as i32;
//         let r = if ch == 2 {
//             let j = i + 2;
//             i16::from_le_bytes([tmp[j], tmp[j+1]]) as i32
//         } else {
//             l
//         };
//         let lu = (l as u32) & 0xFFFF;
//         let ru = (r as u32) & 0xFFFF;
//
//         out_frames[produced] = (lu << 16) | ru;
//         produced += 1;
//         i += bytes_per_sample * ch; // advance by one sample frame in input
//     }
//
//     produced
// }

// Dummy time source required by embedded-sdmmc
#[derive(Default)]
struct DummyTime;
impl embedded_sdmmc::TimeSource for DummyTime {
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

// ----------------- MAIN -----------------
#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    esp_alloc::heap_allocator!(size: 72 * 1024);

    // let p = Peripherals::take();
    // let system = p.SYSTEM.split();
    // let clocks = ClockControl::max(system.clock_control).freeze();
    // let _tg0 = TimerGroup::new(p.TIMG0, &clocks);

    // let io = Io::new(p.GPIO, p.IO_MUX);
    let mut delay = Delay::new();

    // --- SPI2 for SD card (T-Deck pins) ---
    let BOARD_SDCARD_CS = peripherals.GPIO39;
    let sck  = peripherals.GPIO40;
    let mosi = peripherals.GPIO41;
    let miso = peripherals.GPIO38;
    // let cs   = peripherals.gpio41;
    let sdmmc_cs = Output::new(BOARD_SDCARD_CS, High, OutputConfig::default());

    let spi = Spi::new(peripherals.SPI2,
                       SpiConfig::default().with_frequency(Rate::from_mhz(40)), // .with_mode()
    ).unwrap()
        .with_sck(sck)
        .with_mosi(mosi)
        .with_miso(miso);
        // .with_cs(cs);
                       // SpiMode::Mode0, &clocks)
        // .with_pins(sck, mosi, miso, cs);

    // --- Create an SDSPI BlockDevice and embedded-sdmmc Controller ---
    // Plug in your BlockDevice implementation here:
    // let sddev = sdspi::SdSpiDev::new(spi); // <--- implement / import this (read-only is enough)
    let sddev =
        ExclusiveDevice::new_no_delay(spi, sdmmc_cs).expect("Failed to create SpiDevice");

    let mut ctrl = SdCard::new(sddev, delay);
    info!("opening volume manager");
    let mut volume_mgr = VolumeManager::new(ctrl,DummyTime{});
    info!("opening volume");
    let mut volume = volume_mgr.open_volume(VolumeIdx(0)).unwrap();
    info!("opening root dir");
    let root_dir = volume.open_root_dir().unwrap();

    // Open your WAV file (8.3 name unless you enable long names)
    let mut file = root_dir.open_file_in_dir(
        "TEST.WAV", // "TEST.WAV" as 8.3 (pad with spaces)
        embedded_sdmmc::Mode::ReadOnly,
    ).unwrap();

    // Read and parse header
    let mut hdr = [0u8; WAV_HEADER_LEN];
    // ctrl.read(&mut volume, &mut file, &mut hdr).unwrap();
    let wav = parse_wav_header(&hdr).expect("Unsupported WAV header");
    assert_eq!(wav.audio_format, 1);
    assert_eq!(wav.bits_per_sample, 16);
    assert!(wav.num_channels == 1 || wav.num_channels == 2);

    info!("read the wave file");
    // // --- I2S0 TX to built-in speaker pins ---
    // let bclk = io.pins.gpio42;
    // let ws   = io.pins.gpio1;
    // let dout = io.pins.gpio2;
    //
    // let i2s = I2s::new(p.I2S0, &clocks);
    // let tx_pins = PinsBclkWsDout::new(bclk, ws, dout);
    //
    // let dma = Dma::new(p.DMA);
    // let dma_ch = dma.channel0;
    //
    // let mut tx: I2sTx<_> = i2s
    //     .tx_channel(
    //         dma_ch,
    //         tx_pins,
    //         Standard::Philips,
    //         I2sChannel::Stereo,
    //         I2sDataFmt::Data16Channel16,
    //         wav.sample_rate.Hz(),
    //         DmaPriority::Priority0,
    //     )
    //     .unwrap();
    //
    // Queue to record which buffer was just submitted
    let (mut prod, mut cons) = unsafe {
        let q = Q_STORAGE.write(Queue::new());
        q.split()
    };

    // unsafe {
    //     let (a, b) = (&mut DMA_BUF_A.0, &mut DMA_BUF_B.0);
    //
    //     // Preload both buffers
    //     let a_len = fill_frames_from_sd(&mut ctrl, &mut volume, &mut file, a, &wav);
    //     let b_len = fill_frames_from_sd(&mut ctrl, &mut volume, &mut file, b, &wav);
    //     if a_len == 0 {
    //         loop {} // empty file
    //     }
    //
    //     // Kick off DMA with buffer A
    //     tx.write_dma(&a[..a_len]).unwrap();
    //     prod.enqueue(0).ok();
    //
    //     let mut next_is_a = false; // after A, we'll submit B, etc.
    //
    //     loop {
    //         // Wait until TX channel is idle (previous DMA buffer has finished)
    //         if tx.is_tx_idle() {
    //             // Refill the buffer we just finished (the opposite of what we submit next)
    //             if next_is_a {
    //                 // we're about to submit A; refill B
    //                 let b_filled = fill_frames_from_sd(&mut ctrl, &mut volume, &mut file, b, &wav);
    //                 // If EOF, you can break or loop by seeking to start again
    //                 let submit_len = if a_len == 0 { 0 } else { a_len };
    //                 if submit_len == 0 { break; }
    //                 tx.write_dma(&a[..submit_len]).unwrap();
    //                 prod.enqueue(0).ok();
    //             } else {
    //                 // we're about to submit B; refill A
    //                 let a_filled = fill_frames_from_sd(&mut ctrl, &mut volume, &mut file, a, &wav);
    //                 let submit_len = if b_len == 0 { 0 } else { b_len };
    //                 if submit_len == 0 { break; }
    //                 tx.write_dma(&b[..submit_len]).unwrap();
    //                 prod.enqueue(1).ok();
    //             }
    //             next_is_a = !next_is_a;
    //         }
    //     }
    // }

    loop {}
}

// ---- SDSPI glue stub ----
// Replace with your existing SDSPI driver that implements BlockDevice.
// Any driver that can satisfy embedded_sdmmc::BlockDevice (read-only) will work.
// mod sdspi {
//     use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx, Error as SdError, SdCardError};
//
//     pub struct SdSpiDev<SPI> {
//         spi: SPI,
//         // Add CS pin control + delays + card init state here
//     }
//     impl<SPI> SdSpiDev<SPI> {
//         pub fn new(spi: SPI) -> Self { Self { spi } }
//     }
//
//     impl<SPI> BlockDevice for SdSpiDev<SPI>
//     where
//         SPI: SpiBus<u8>,
//     {
//         type Error = SdError<SdCardError>;
//
//         fn read(&mut self, _blocks: &mut [Block], _start_block: BlockIdx, _reason: &str)
//                 -> Result<(), Self::Error>
//         {
//             // TODO: implement CMD17/CMD18 single/multi-block reads in SPI mode
//             // Or swap this module with an existing SDSPI BlockDevice crate.
//             unimplemented!()
//         }
//
//         fn write(&mut self, _blocks: &[Block], _start_block: BlockIdx)
//                  -> Result<(), Self::Error>
//         {
//             // Not needed for playback; can be left unimplemented for read-only
//             unimplemented!()
//         }
//
//         fn num_blocks(&mut self) -> Result<BlockCount, Self::Error> {
//             // Optional: return card size if your driver tracks it
//             Err(SdError::DeviceError(SdCardError::GenericError))
//         }
//     }
// }