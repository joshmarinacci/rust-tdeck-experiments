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

use alloc::vec::Vec;
use esp_backtrace as _;
use core::cell::RefCell;
use core::hash::Hash;
use core::ops::Deref;
use embedded_storage::{ReadStorage, Storage};
use esp_bootloader_esp_idf::partitions;
use esp_bootloader_esp_idf::partitions::PartitionType;
use esp_hal::clock::CpuClock;
use esp_hal::{main, peripherals};
use esp_hal::time::{Duration, Instant};
use esp_storage::FlashStorage;
use log::info;
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};

// #[panic_handler]
// fn panic(_: &core::panic::PanicInfo) -> ! {
//     loop {}
// }

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

extern crate alloc;

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let mut flash = FlashStorage::new(peripherals.FLASH);
    info!("Flash size = {}", flash.capacity());

    let mut pt_mem = [0u8; partitions::PARTITION_TABLE_MAX_LEN];
    let pt = partitions::read_partition_table(&mut flash, &mut pt_mem).unwrap();

    for i in 0..pt.len() {
        let raw = pt.get_partition(i).unwrap();
        info!("partition {:?}", raw);
    }
    info!("done reading flash");

    let nvs = pt
        .find_partition(PartitionType::Data(partitions::DataPartitionSubType::Nvs)).unwrap().unwrap();

    let mut nvs_partition = nvs.as_embedded_storage(&mut flash);

    /*
    let mut bytes = [0u8; 32];
    info!("NVS partition size = {}", nvs_partition.capacity());

    let offset_in_nvs_partition = 0;

    nvs_partition
        .read(offset_in_nvs_partition, &mut bytes)
        .unwrap();
    info!(
        "Read from {:x}:  {:02x?}",
        offset_in_nvs_partition,
        &bytes[..32]
    );

    bytes[0x00] = bytes[0x00].wrapping_add(1);
    bytes[0x01] = bytes[0x01].wrapping_add(2);
    bytes[0x02] = bytes[0x02].wrapping_add(3);
    bytes[0x03] = bytes[0x03].wrapping_add(4);
    bytes[0x04] = bytes[0x04].wrapping_add(1);
    bytes[0x05] = bytes[0x05].wrapping_add(2);
    bytes[0x06] = bytes[0x06].wrapping_add(3);
    bytes[0x07] = bytes[0x07].wrapping_add(4);

    nvs_partition
        .write(offset_in_nvs_partition, &bytes)
        .unwrap();
    info!(
        "Written to {:x}: {:02x?}",
        offset_in_nvs_partition,
        &bytes[..32]
    );

    let mut reread_bytes = [0u8; 32];
    nvs_partition.read(0, &mut reread_bytes).unwrap();
    info!(
        "Read from {:x}:  {:02x?}",
        offset_in_nvs_partition,
        &reread_bytes[..32]
    );

    info!("Reset (CTRL-R in espflash) to re-read the persisted data.");
*/


    let mut bytes = [0u8; 32];
    info!("partition size = {}", nvs_partition.capacity());

    let offset_in_nvs_partition = 0;
    nvs_partition.read(offset_in_nvs_partition, &mut bytes).unwrap();
    info!("read from {:x}: {:02x?}", offset_in_nvs_partition, &bytes[..32]);

    let read_result = from_bytes::<UserInfo>(&bytes);
    info!("got back {:?}",read_result);
    if let Ok(user_info) = read_result {
        info!("read back user info: {:?}", user_info);
    } else {
        info!("error reading. writeing new data");
        let user_info = UserInfo {
            first: "josh",
            last: "marinacci",
        };
        info!("making a user info");
        let output: Vec<u8> = to_allocvec(&user_info).unwrap();
        info!("size of the user info is {}",output.len());
        info!("user info bytes is {:02x?}", &output);
        info!("copying from output");

        let ui = from_bytes::<UserInfo>(&output);
        info!("read back bytes {:?}",ui);

        for i in 0..output.len() {
            bytes[i] = output[i];
        }
        info!("writing back bytes {:02x?}",bytes);
        let write_result = nvs_partition.write(offset_in_nvs_partition, &bytes);
        // let write_result = nvs_partition.write(offset_in_nvs_partition, &output[..32]);
        info!("write back result: {:?}", write_result);
        write_result.unwrap();
        let mut reread_bytes = [0u8; 32];
        nvs_partition.read(0, &mut reread_bytes).unwrap();
        info!(
            "Read from {:x}:  {:02x?}",
            offset_in_nvs_partition,
            &reread_bytes[..32]
        );
    }

    loop { }
}


#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct UserInfo<'a> {
    first:&'a str,
    last:&'a str,
}