#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
use esp_hal::clock::CpuClock;
use esp_hal::{chip, main};
use esp_hal::time::{Duration, Instant};
use esp_hal::efuse::Efuse;
use log::info;

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
    esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("running");

    loop {
        info!("getting information!");
        info!("flash encryption? {}", Efuse::flash_encryption());
        info!("block version? {:?}", Efuse::block_version());
        info!("mac address? {:?}", Efuse::mac_address());
        info!("chip name {:?}", chip!());
        info!("Free memory {:?}",  esp_alloc::HEAP.free());
        info!("Used memory {:?}",  esp_alloc::HEAP.used());
        info!("HEAP stats {}",  esp_alloc::HEAP.stats());
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}