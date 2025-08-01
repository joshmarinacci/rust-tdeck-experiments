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

use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::efuse::Efuse;
use esp_hal::time::{Duration, Instant, Rate};
use esp_hal::{chip, main};
use log::info;
use embedded_sdmmc::{SdCard, VolumeManager, Mode, VolumeIdx, TimeSource, Timestamp};
use esp_hal::delay::Delay;
use esp_hal::gpio::{Output, OutputConfig, OutputPin};
use esp_hal::gpio::Level::Low;
use esp_hal::spi::{ master::{Spi, Config as SpiConfig } };
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

    info!("running");
    let mut delay = Delay::new();

    info!("connecting to the bus");

    let sdmmc_cs = Output::new(peripherals.GPIO39, Low, OutputConfig::default());
    let sdmmc_spi_bus = Spi::new(peripherals.SPI2,
                                 SpiConfig::default()
                                     .with_frequency(Rate::from_mhz(40))
                                     // .with_mode()
                                     // .with_clock_source(SpiClockSource::LSI)
    ).unwrap();
// https://github.com/Xinyuan-LilyGO/T-Deck/blob/master/examples/I2SPlay/utilities.h
//         .with_sck(peripherals.GPIO40)
//         .with_miso(peripherals.GPIO38)
//         .with_mosi(peripherals.GPIO41)
        // .with_miso(peripherals.GPIO39)
        ;
    info!("setup pins");

    let sdmmc_spi = ExclusiveDevice::new_no_delay(sdmmc_spi_bus, sdmmc_cs).expect("Failed to create SpiDevice");
    info!("open the card");
    let card = SdCard::new(sdmmc_spi, delay);
    info!("open the volume manager");

    let mut volume_mgr = VolumeManager::new(card, DummyTimesource {});
    info!("getting volume");

    // // Try and access Volume 0 (i.e. the first partition).
    // // The volume object holds information about the filesystem on that volume.
    let volume = volume_mgr
        .open_volume(VolumeIdx(0))
        .expect("Failed to open volume");
    info!("set up the sdmmc_spi");
    info!("volume 0: is {:?}",volume);


    // // Open the root directory (mutably borrows from the volume).
    // let root_dir = volume0.open_root_dir()?;
    // // Open a file called "MY_FILE.TXT" in the root directory
    // // This mutably borrows the directory.
    // let my_file = root_dir.open_file_in_dir("MY_FILE.TXT", Mode::ReadOnly)?;
    // // Print the contents of the file, assuming it's in ISO-8859-1 encoding
    // while !my_file.is_eof() {
    //     let mut buffer = [0u8; 32];
    //     let num_read = my_file.read(&mut buffer)?;
    //     for b in &buffer[0..num_read] {
    //         info!("{}", *b as char);
    //     }
    // }
    //
    // let mut flash = FlashStorage::new();
    // let flash_addr = 0x9000;
    // info!("Flash size = {}", flash.capacity());
    loop {
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}
