#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::{main, Blocking};
use esp_hal::spi::master::{Config as SpiConfig, Spi};
use esp_hal::time::{Duration, Instant, Rate};
use log::info;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
};
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_sdmmc::{SdCard, TimeSource, Timestamp, VolumeIdx, VolumeManager};
use embedded_sdmmc::Mode::{ReadWriteCreateOrAppend, ReadWriteCreateOrTruncate};
use mipidsi::interface::SpiInterface;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use mipidsi::{models::ST7789, Builder, Display, NoResetPin};

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let mut delay = Delay::new();

    let BOARD_POWERON = peripherals.GPIO10;

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(BOARD_POWERON, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);

    // ==== display setup ====
    // https://github.com/Xinyuan-LilyGO/T-Deck/blob/master/examples/HelloWorld/HelloWorld.ino
    let board_sdcard_cs = peripherals.GPIO39;
    let radio_cs_pin = peripherals.GPIO9;
    let board_tft_cs = peripherals.GPIO12;
    let board_spi_miso = peripherals.GPIO38;

    let board_spi_sck = peripherals.GPIO40;
    let board_spi_mosi = peripherals.GPIO41;

    let sdmmc_cs = Output::new(board_sdcard_cs, High, OutputConfig::default());
    let radio_cs = Output::new(radio_cs_pin, High, OutputConfig::default());
    let board_tft = Output::new(board_tft_cs, High, OutputConfig::default());

    let board_spi_miso = Input::new(board_spi_miso, InputConfig::default().with_pull(Pull::Up));

    let sdmmc_spi_bus = Spi::new(
        peripherals.SPI2,
        SpiConfig::default().with_frequency(Rate::from_mhz(40)), // .with_mode()
        // .with_clock_source(SpiClockSource::LSI)
    )
        .unwrap()
        .with_sck(board_spi_sck)
        .with_miso(board_spi_miso)
        .with_mosi(board_spi_mosi);
    let sdmmc_spi =
        ExclusiveDevice::new_no_delay(sdmmc_spi_bus, sdmmc_cs).expect("Failed to create SpiDevice");
    info!("open the card");
    let card = SdCard::new(sdmmc_spi, delay);


    info!("initialized display");

    let mut volume_mgr = VolumeManager::new(card, DummyTimesource{}); // Use your TimeSource

    info!("drawing to buffer");
    draw_to_buffer(&mut volume_mgr);

    loop {
        info!("sleeping");
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}

struct ExampleDisplay {
    framebuffer: [u8; 20*20*3],
}

impl OriginDimensions for ExampleDisplay {
    fn size(&self) -> Size {
        Size::new(20,20)
    }
}
impl DrawTarget for ExampleDisplay {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item=Pixel<Self::Color>>
    {
        for Pixel(coord, color) in pixels.into_iter() {
            let width = self.size().width;
            let height = self.size().height;
            let x = coord.x as u32;
            let y = coord.y as u32;
            let index: u32 = (x + y * width)*3;
            self.framebuffer[(index+0) as usize] = color.r()<<3;
            self.framebuffer[(index+1) as usize] = color.g()<<2;
            self.framebuffer[(index+2) as usize] = color.b()<<3;
        }
        Ok(())
    }
}
fn draw_to_buffer(volume_mgr: &mut VolumeManager<SdCard<ExclusiveDevice<Spi<Blocking>, Output, NoDelay>, Delay>, DummyTimesource>) {
    const WIDTH:usize = 20;
    const HEIGHT:usize = 20;
    let mut buffer = ExampleDisplay {
        framebuffer: [0; WIDTH*HEIGHT*3]
    };
    Rectangle::new(Point::new(0,0), Size::new(10,10))
        .into_styled(PrimitiveStyle::with_fill(Rgb565::MAGENTA))
        .draw(&mut buffer).unwrap();
    info!("the first few pixels are {:?}", &buffer.framebuffer[0 .. 10]);

    info!("Making a bmp");
    // --- BMP Encoding and Writing ---
    // You would typically use `tinybmp` to encode your raw pixel data into BMP format
    // This is a simplified example, you'll need to adapt it.
    let bmp_bytes = {
        // Example: Create a BMP from raw data (replace with actual tinybmp usage)
        // tinybmp might provide a builder or a function to create a Bmp from raw data
        // For simplicity, let's assume we have a pre-encoded BMP file in memory
        // This is where you'd use tinybmp::Bmp::from_pixel_data() or similar
        // For now, let's just make a dummy header and data for demonstration
        let mut dummy_bmp_data = [0u8; 54 + (WIDTH * HEIGHT * 3) as usize]; // Simple BMP header + pixel data
        info!("allocated bmp data");
        // Fill in BMP header (this is highly simplified and incomplete!)
        dummy_bmp_data[0] = b'B';
        dummy_bmp_data[1] = b'M';
        // ... fill other header fields with appropriate values ...
        dummy_bmp_data[10] = 54; // Data offset
        dummy_bmp_data[14] = 40; // Header size
        info!("copying width");
        dummy_bmp_data[18..22].copy_from_slice(&WIDTH.to_le_bytes());
        info!("copying height");
        dummy_bmp_data[22..26].copy_from_slice(&HEIGHT.to_le_bytes());
        dummy_bmp_data[26] = 1; // Planes
        dummy_bmp_data[28] = 24; // Bits per pixel (RGB888)

        info!("copying rest of the bytes");
        dummy_bmp_data[54..].copy_from_slice(&buffer.framebuffer); // Copy pixel data after header

        info!("turning into a vec");
        dummy_bmp_data.to_vec() // Convert to Vec<u8> (requires alloc feature)
        // Or use a fixed-size buffer if alloc is not available
    };

    info!("prepared a bmp file");// {:?}", bmp_bytes);


    info!("opening volume");
    let vol = volume_mgr.open_volume(VolumeIdx(0)).unwrap();
    info!("opening root dir");
    let root = vol.open_root_dir().unwrap();
    let file = root.open_file_in_dir("IMAGE.BMP",ReadWriteCreateOrTruncate).unwrap();
    match file.write(&bmp_bytes) {
        Ok(_) => {
            info!("wrote out IMAGE.BMP");
            // Success!
            // You might want to flush the file here, embedded-sdmmc handles flushing on close.
            file.close().unwrap();
        },
        Err(e) => {
            info!("error writing image {:?}",e);
            // Handle write error
            // log::error!("Failed to write BMP data: {:?}", e);
        }
    }
}

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
