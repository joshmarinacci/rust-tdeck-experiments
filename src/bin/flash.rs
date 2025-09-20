/*
Derived from the official example:
    https://github.com/esp-rs/esp-hal/blob/main/examples/peripheral/flash_read_write/src/main.rs

This example accesses the built-in flash storage
prints the capacity in bytes
and prints out the partition table.
 */
#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use embedded_storage::ReadStorage;
use esp_bootloader_esp_idf::partitions;
use esp_hal::clock::CpuClock;
use esp_hal::main;
use esp_hal::time::{Duration, Instant};
use esp_storage::FlashStorage;
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

    let mut flash = FlashStorage::new();
    info!("Flash size = {}", flash.capacity());

    let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
    let pt = partitions::read_partition_table(&mut flash, &mut pt_mem).unwrap();

    for i in 0..pt.len() {
        let raw = pt.get_partition(i).unwrap();
        info!("{:?}", raw);
    }
    info!("done reading flash");

    loop {
        let delay_start = Instant::now();
        while delay_start.elapsed() < Duration::from_millis(500) {}
    }
}
