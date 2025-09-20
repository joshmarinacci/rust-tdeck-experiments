#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use embassy_executor::Spawner;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use embedded_sdmmc::{BlockDevice, File, SdCard, Timestamp, VolumeIdx, VolumeManager};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::dma::DmaTransferTx;
use esp_hal::gpio::Level::High;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::i2s::master::{DataFormat, Error, I2s, I2sTx, Standard};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::Rate;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{dma_buffers, dma_circular_buffers, main, Blocking};
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
const CHUNK_BYTES: usize = 8192 * 2; // increase if you get underruns
                                     // const CHUNK_BYTES: usize = 48000; // increase if you get underruns
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
// static mut DMA_BUF_A: Aligned<[u32; CHUNK_BYTES / 4]> = Aligned([0; CHUNK_BYTES / 4]);
// static mut DMA_BUF_B: Aligned<[u32; CHUNK_BYTES / 4]> = Aligned([0; CHUNK_BYTES / 4]);

// Queue to track which buffer is currently playing (0 or 1)
// static mut Q_STORAGE: MaybeUninit<Queue<u8, 4>> = MaybeUninit::uninit();

// ---------- WAV info ----------
#[derive(Clone, Copy, Debug)]
struct WavInfo {
    audio_format: u16, // 1 = PCM
    num_channels: u16, // 1 or 2
    sample_rate: u32,
    bits_per_sample: u16, // 16
    data_size: u32,       // bytes
    block_align: u16,     // bytes per frame (channels * bits/8)
}

// Minimal parser for 16-bit PCM, finds the "fmt " & "data" chunks in the 44B header
fn parse_wav_header(h: &[u8]) -> Option<WavInfo> {
    use byteorder::{ByteOrder, LittleEndian as LE};
    if h.len() < 44 || &h[0..4] != b"RIFF" || &h[8..12] != b"WAVE" {
        return None;
    }

    let mut p = 12usize;
    let mut fmt_found = false;
    let mut audio_format = 0u16;
    let mut num_channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut block_align = 0u16;
    let mut data_size = 0u32;

    while p + 8 <= h.len() {
        let id = &h[p..p + 4];
        let sz = LE::read_u32(&h[p + 4..p + 8]) as usize;
        let body = p + 8;
        let next = body + sz;

        if id == b"fmt " && body + 16 <= h.len() {
            audio_format = LE::read_u16(&h[body..body + 2]);
            num_channels = LE::read_u16(&h[body + 2..body + 4]);
            sample_rate = LE::read_u32(&h[body + 4..body + 8]);
            let _byte_rate = LE::read_u32(&h[body + 8..body + 12]);
            block_align = LE::read_u16(&h[body + 12..body + 14]);
            bits_per_sample = LE::read_u16(&h[body + 14..body + 16]);
            fmt_found = true;
        } else if id == b"data" {
            data_size = (sz as u32);
            break;
        }
        // chunks are word-aligned
        p = next + (next & 1);
    }

    if !fmt_found || data_size == 0 {
        return None;
    }
    Some(WavInfo {
        audio_format,
        num_channels,
        sample_rate,
        bits_per_sample,
        data_size,
        block_align,
    })
}

// Read PCM bytes from SD and pack to u32 I2S frames (L: high 16, R: low 16)
fn fill_frames_from_sd(
    file: &mut File<
        SdCard<ExclusiveDevice<Spi<Blocking>, Output, NoDelay>, Delay>,
        DummyTime,
        4,
        4,
        1,
    >,
    out_frames: &mut [u32],
    w: &WavInfo,
) -> usize {
    // Temp byte buffer: size in bytes equals out_frames*4 (16b stereo = 4 bytes per frame)
    let mut tmp = [0u8; CHUNK_BYTES];

    let bytes_per_sample = (w.bits_per_sample / 8) as usize; // 2
    let ch = w.num_channels as usize; // 1 or 2

    // Compute how many PCM bytes we'd like to fill
    let want_frames = out_frames.len();
    let want_pcm_bytes = want_frames * bytes_per_sample * ch;
    let to_read = core::cmp::min(tmp.len(), want_pcm_bytes);

    if file.is_eof() {
        // info!("end of file");
        file.seek_from_start(WAV_HEADER_LEN as u32).unwrap();
    }
    let n = file.read(&mut tmp[..to_read]).unwrap_or(0);
    if n == 0 {
        return 0;
    }

    let mut produced = 0usize;
    let mut i = 0usize;
    while i + bytes_per_sample * ch <= n && produced < out_frames.len() {
        // 16-bit little-endian
        let l = i16::from_le_bytes([tmp[i], tmp[i + 1]]) as i32;
        let r = if ch == 2 {
            let j = i + 2;
            i16::from_le_bytes([tmp[j], tmp[j + 1]]) as i32
        } else {
            l
        };
        let lu = (l as u32) & 0xFFFF;
        let ru = (r as u32) & 0xFFFF;

        out_frames[produced] = (lu << 16) | ru;
        produced += 1;
        i += bytes_per_sample * ch; // advance by one sample frame in input
    }

    produced
}

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
#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    // setup
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let timer_g1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer_g1.timer0);
    esp_alloc::heap_allocator!(size: 96 * 1024);
    let delay = Delay::new();

    // poweron board
    let BOARD_POWERON = peripherals.GPIO10;
    let BOARD_SDCARD_CS = peripherals.GPIO39;
    let mut board_power = Output::new(BOARD_POWERON, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    // --- SPI2 for SD card (T-Deck pins) ---
    let BOARD_SPI_SCK = peripherals.GPIO40;
    let BOARD_SPI_MOSI = peripherals.GPIO41;
    let BOARD_SPI_MISO = peripherals.GPIO38;
    let RADIO_CS_PIN = peripherals.GPIO9;
    let BOARD_TFT_CS = peripherals.GPIO12;
    // let cs   = peripherals.gpio41;
    let sdmmc_cs = Output::new(BOARD_SDCARD_CS, High, OutputConfig::default());
    let BOARD_SPI_MISO = Input::new(BOARD_SPI_MISO, InputConfig::default().with_pull(Pull::Up));
    // I don't know why we need these set, but we do.
    let radio_cs = Output::new(RADIO_CS_PIN, High, OutputConfig::default());
    let board_tft = Output::new(BOARD_TFT_CS, High, OutputConfig::default());

    let sdmmc_spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)),
    )
    .unwrap()
    .with_sck(BOARD_SPI_SCK)
    .with_mosi(BOARD_SPI_MOSI)
    .with_miso(BOARD_SPI_MISO);
    let sdmmc_spi =
        ExclusiveDevice::new_no_delay(sdmmc_spi_bus, sdmmc_cs).expect("Failed to create SpiDevice");

    // setup SD Card for reading FAT
    let card = SdCard::new(sdmmc_spi, delay);
    info!("size of card in bytes: {}", card.num_bytes().unwrap());
    info!("type of card: {:?}", card.get_card_type());
    info!("opening volume manager");
    let mut volume_mgr = VolumeManager::new(card, DummyTime {});
    info!("opening volume");
    let mut volume = volume_mgr.open_volume(VolumeIdx(0)).unwrap();
    info!("opening root dir");
    let root_dir = volume.open_root_dir().unwrap();

    // get file for WAV file
    let mut file = root_dir
        .open_file_in_dir("U2MYST.WAV", embedded_sdmmc::Mode::ReadOnly)
        .unwrap();

    info!("opened the file {:?}", file);

    // Read and parse WAV header
    let mut hdr = [0u8; WAV_HEADER_LEN];
    file.read(&mut hdr).unwrap();
    let wav = parse_wav_header(&hdr).expect("Unsupported WAV header");
    assert_eq!(wav.audio_format, 1);
    assert_eq!(wav.bits_per_sample, 16);
    assert!(wav.num_channels == 1 || wav.num_channels == 2);

    info!("read the wave file");
    info!("wav {:?}", wav);

    // test_sdcard_reading(&mut file, &wav);

    // // create DMA buffers
    let (_, _, tx_buffer, tx_descriptors) = dma_circular_buffers!(0, CHUNK_BYTES);

    // SETUP I2S
    // --- I2S0 TX to built-in speaker pins ---
    let bclk = peripherals.GPIO7;
    let ws = peripherals.GPIO5;
    let dout = peripherals.GPIO6;
    let i2s = I2s::new(
        peripherals.I2S0,
        Standard::Philips,
        DataFormat::Data16Channel16,
        Rate::from_hz(44100),
        peripherals.DMA_CH0,
    );
    let mut i2s_tx = i2s
        .into_async()
        .i2s_tx
        .with_bclk(bclk)
        .with_ws(ws)
        .with_dout(dout)
        .build(tx_descriptors);

    // create temp buffers
    let mut file_buffer = [0u32; CHUNK_BYTES / 2];

    info!("tx buffer is {}", tx_buffer.len());
    info!("CHUNK_BYTES is {}", CHUNK_BYTES);

    // setup circular buffer writing
    let mut transaction = i2s_tx.write_dma_circular_async(tx_buffer).unwrap();
    loop {
        transaction
            .push_with(|dma_buf| {
                // len = length of dma_buffer in bytes
                let i32_len = file_buffer.len();
                // use the shorter of dma and file buffer lengths
                let file_len = (dma_buf.len() / 4).min(i32_len);
                // file_buffer is u32 so we use 1/4 of the len
                let slice = &mut file_buffer[0..file_len];
                fill_frames_from_sd(&mut file, slice, &wav);
                for (dest_byte, source_i32) in dma_buf.chunks_exact_mut(4).zip(slice.iter()) {
                    dest_byte.copy_from_slice(&source_i32.to_le_bytes())
                }
                file_len * 4
            })
            .await
            .unwrap();
    }
}

fn test_sdcard_reading(
    file: &mut File<
        SdCard<ExclusiveDevice<Spi<Blocking>, Output, NoDelay>, Delay>,
        DummyTime,
        4,
        4,
        1,
    >,
    w: &WavInfo,
) {
    info!("testing card read");
    info!("reading {CHUNK_BYTES} at a time");

    let mut out_frames = [0u32; CHUNK_BYTES / 4];
    let mut tmp = [0u8; CHUNK_BYTES];

    let bytes_per_sample = (w.bits_per_sample / 8) as usize; // 2
    info!("bytes per sample {bytes_per_sample}");
    let ch = w.num_channels as usize; // 1 or 2
    info!("channel count {ch}");

    // Compute how many PCM bytes we'd like to fill
    let want_frames = out_frames.len();
    let want_pcm_bytes = want_frames * bytes_per_sample * ch;
    let to_read = core::cmp::min(tmp.len(), want_pcm_bytes);

    let start = embassy_time::Instant::now();
    info!("starting to read at {}", start);

    let mut count = 0;
    let mut total = 0;
    loop {
        // read some bytes
        let n = file.read(&mut tmp[..to_read]).unwrap_or(0);
        total += n;
        if n == 0 {
            info!("read zero bytes");
            break;
        }

        if file.is_eof() {
            info!("end of file");
            break;
        }

        let mut produced = 0usize;
        let mut i = 0usize;
        while i + bytes_per_sample * ch <= n && produced < out_frames.len() {
            // 16-bit little-endian
            let l = i16::from_le_bytes([tmp[i], tmp[i + 1]]) as i32;
            let r = if ch == 2 {
                let j = i + 2;
                i16::from_le_bytes([tmp[j], tmp[j + 1]]) as i32
            } else {
                l
            };
            let lu = (l as u32) & 0xFFFF;
            let ru = (r as u32) & 0xFFFF;

            out_frames[produced] = (lu << 16) | ru;
            produced += 1;
            i += bytes_per_sample * ch; // advance by one sample frame in input
        }
        count += 1;
        if count > 200 {
            break;
        }
        // info!("made a frame");
    }
    info!("read total bytes of {total}");
    let end = embassy_time::Instant::now();
    info!("ending read at {}", end);
    let dur = (end - start).as_millis();
    info!("total ticks is {} msec", dur);
    let bps = (total as u64 * 1000) / dur;
    info!("kbytes per sec {}", bps / 1024);
    //753kb/s over ~4 sec
    //755kb/s over ~8 sec
    //~42mb over at
    // 44.1khz at 16bits per sample, stereo, is 176,400 bytes per second
}
