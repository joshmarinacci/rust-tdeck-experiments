/*
 this doesn't work yet
 */
#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use alloc::string::String;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::efuse::Efuse;
use esp_hal::time::{Duration, Instant, Rate};
use esp_hal::{chip, main};
use log::info;
use embedded_sdmmc::{SdCard, VolumeManager, Mode, VolumeIdx, TimeSource, Timestamp};
use embedded_sdmmc::Mode::ReadOnly;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, OutputPin, Pull};
use esp_hal::gpio::Level::{High, Low};
use esp_hal::spi::{ master::{Spi, Config as SpiConfig } };
use esp_wifi::wifi::event::handle;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

/// A dummy timesource, which is mostly important for creating files.
#[derive(Default)]
pub struct DummyTimesource();

impl TimeSource for DummyTimesource {
    // In theory you could use the RTC of the rp2040 here, if you had
    // any external time synchronizing device.
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
#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let BOARD_POWERON = peripherals.GPIO10;
    let BOARD_SDCARD_CS = peripherals.GPIO39;
    let RADIO_CS_PIN = peripherals.GPIO9;
    let BOARD_TFT_CS = peripherals.GPIO12;
    let BOARD_SPI_MISO = peripherals.GPIO38;

    let BOARD_SPI_SCK = peripherals.GPIO40;
    let BOARD_SPI_MOSI = peripherals.GPIO41;

    info!("running");
    let mut delay = Delay::new();
    let mut board_power = Output::new(BOARD_POWERON, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(2000);

    info!("connecting to the bus");

    let sdmmc_cs = Output::new(BOARD_SDCARD_CS, High, OutputConfig::default());
    let radio_cs = Output::new(RADIO_CS_PIN, High, OutputConfig::default());
    let board_tft = Output::new(BOARD_TFT_CS, High, OutputConfig::default());

    let board_spi_miso = Input::new(BOARD_SPI_MISO, InputConfig::default().with_pull(Pull::Up));

    let sdmmc_spi_bus = Spi::new(peripherals.SPI2,
                                 SpiConfig::default()
                                     .with_frequency(Rate::from_mhz(40))
                                     // .with_mode()
                                     // .with_clock_source(SpiClockSource::LSI)
    ).unwrap()
        .with_sck(BOARD_SPI_SCK)
        .with_miso(board_spi_miso)
        .with_mosi(BOARD_SPI_MOSI)
        ;
    let sdmmc_spi = ExclusiveDevice::new_no_delay(sdmmc_spi_bus, sdmmc_cs).expect("Failed to create SpiDevice");
    info!("open the card");
    let card = SdCard::new(sdmmc_spi, delay);
    info!("open the volume manager");
    let mut volume_mgr = VolumeManager::new(card, DummyTimesource {});
    info!("getting volume");
    match volume_mgr.open_volume(VolumeIdx(0)) {
        Ok(handle) => {
            info!("opened the volume {:?}",handle);
            let root_dir = handle.open_root_dir().unwrap();
            root_dir.iterate_dir(|de|{
                info!("dir entry {:?} is {} bytes",de.name, de.size);
            }).unwrap();
            let my_file = root_dir.open_file_in_dir("README.MD", ReadOnly).unwrap();
            while !my_file.is_eof() {
                let mut buffer = [0u8; 32];
                let num_read = my_file.read(&mut buffer).unwrap();
                let slice = &buffer[0..num_read];
                let line = String::from_utf8_lossy(slice);
                info!("{}",line);
            }
            my_file.close().unwrap();
            root_dir.close().unwrap();
            handle.close().unwrap();
        }
        Err(err) => {
            info!("failed to open the volume {:?}",err);
        }
    }
    loop {
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(5000) {}
    }
}
