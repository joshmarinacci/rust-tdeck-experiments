/*

This example creates a USB HID device (a virtual keyboard)
and sends the A keystroke every five seconds.

Note that this will shut down the built-in serial port being used for
console output, so you will need to enter bootloader mode again to reflash
your firmware.


 */
#![no_std]
#![no_main]

use core::sync::atomic::{AtomicBool, Ordering};
use embassy_executor::Spawner;
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use embassy_usb::{Builder, class::cdc_acm::{CdcAcmClass, State}, driver::EndpointError, Handler};
use esp_backtrace as _;
use esp_hal::{
    otg_fs::{
        Usb,
        asynch::{Config, Driver},
    },
    timer::timg::TimerGroup,
};
use embassy_usb::class::hid::{HidReaderWriter, ReportId, RequestHandler, State as HidState};
use embassy_usb::control::OutResponse;
use log::{info, warn};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    esp_println::println!("Init!");

    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default());

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(
        timg0.timer0,
    );

    let usb = Usb::new(peripherals.USB0, peripherals.GPIO20, peripherals.GPIO19);

    // Create the driver, from the HAL.
    let mut ep_out_buffer = [0u8; 1024];
    let config = Config::default();

    let driver = Driver::new(usb, &mut ep_out_buffer, config);

    // Create embassy-usb Config
    let mut config =  embassy_usb::Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("Embassy");
    config.product = Some("HID keyboard example");
    config.serial_number = Some("12345678");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    // Required for windows compatibility.
    // https://developer.nordicsemi.com/nRF_Connect_SDK/doc/1.9.1/kconfig/CONFIG_CDC_ACM_IAD.html#help
    config.device_class = 0xEF;
    config.device_sub_class = 0x02;
    config.device_protocol = 0x01;
    config.composite_with_iads = true;


    // Create embassy-usb DeviceBuilder using the driver and config.
    // It needs some buffers for building the descriptors.
    let mut config_descriptor = [0; 256];
    let mut bos_descriptor = [0; 256];
    // You can also add a Microsoft OS descriptor.
    let mut msos_descriptor = [0; 256];
    let mut control_buf = [0; 64];
    let mut request_handler = MyRequestHandler {};
    let mut device_handler = MyDeviceHandler::new();

    // let mut state = State::new();

    let mut state = HidState::new();


    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut [], // no msos descriptors
        &mut control_buf,
    );

    let config = embassy_usb::class::hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 60,
        max_packet_size: 64,
    };
    let hid = HidReaderWriter::<_, 1, 8>::new(&mut builder, &mut state, config);

    // Build the builder.
    let mut usb = builder.build();

    // Run the USB device.
    let usb_fut = usb.run();

    let (reader, mut writer) = hid.split();
    // Do stuff with the class!
    let in_fut = async {
        loop {
            info!("waiting 5 sec");
            Timer::after(Duration::from_millis(5000)).await;
            // signal_pin.wait_for_high().await;
            info!("HIGH DETECTED");
            // Create a report with the A key pressed. (no shift modifier)a
            let report = KeyboardReport {
                keycodes: [4, 0, 0, 0, 0, 0],
                leds: 0,
                modifier: 0,
                reserved: 0,
            };
            // Send the report.
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
            // signal_pin.wait_for_low().await;
            info!("waiting 5 sec");
            Timer::after(Duration::from_millis(5000)).await;
            info!("LOW DETECTED");
            let report = KeyboardReport {
                keycodes: [0, 0, 0, 0, 0, 0],
                leds: 0,
                modifier: 0,
                reserved: 0,
            };
            match writer.write_serialize(&report).await {
                Ok(()) => {}
                Err(e) => warn!("Failed to send report: {:?}", e),
            };
        }
    };

    let out_fut = async {
        reader.run(false, &mut request_handler).await;
    };

    // Run everything concurrently.
    // If we had made everything `'static` above instead, we could do this using separate tasks instead.
    join(usb_fut, join(in_fut, out_fut)).await;

}

struct MyRequestHandler {}

impl RequestHandler for MyRequestHandler {
    fn get_report(&mut self, id: ReportId, _buf: &mut [u8]) -> Option<usize> {
        info!("Get report for {:?}", id);
        None
    }

    fn set_report(&mut self, id: ReportId, data: &[u8]) -> OutResponse {
        // info!("Set report for {:?}: {[u8]}", id, data);
        OutResponse::Accepted
    }

    fn set_idle_ms(&mut self, id: Option<ReportId>, dur: u32) {
        info!("Set idle rate for {:?} to {:?}", id, dur);
    }

    fn get_idle_ms(&mut self, id: Option<ReportId>) -> Option<u32> {
        info!("Get idle rate for {:?}", id);
        None
    }
}

async fn echo<'d>(class: &mut CdcAcmClass<'d, Driver<'d>>) -> Result<(), Disconnected> {
    let mut buf = [0; 64];
    loop {
        let n = class.read_packet(&mut buf).await?;
        // Echo back in upper case
        for c in buf[0..n].iter_mut() {
            if 0x61 <= *c && *c <= 0x7a {
                *c &= !0x20;
            }
        }
        let data = &buf[..n];
        class.write_packet(data).await?;
    }
}

struct Disconnected {}

impl From<EndpointError> for Disconnected {
    fn from(val: EndpointError) -> Self {
        match val {
            EndpointError::BufferOverflow => panic!("Buffer overflow"),
            EndpointError::Disabled => Disconnected {},
        }
    }
}


struct MyDeviceHandler {
    configured: AtomicBool,
}

impl MyDeviceHandler {
    fn new() -> Self {
        MyDeviceHandler {
            configured: AtomicBool::new(false),
        }
    }
}

impl Handler for MyDeviceHandler {
    fn enabled(&mut self, enabled: bool) {
        self.configured.store(false, Ordering::Relaxed);
        if enabled {
            info!("Device enabled");
        } else {
            info!("Device disabled");
        }
    }

    fn reset(&mut self) {
        self.configured.store(false, Ordering::Relaxed);
        info!("Bus reset, the Vbus current limit is 100mA");
    }

    fn addressed(&mut self, addr: u8) {
        self.configured.store(false, Ordering::Relaxed);
        info!("USB address set to: {}", addr);
    }

    fn configured(&mut self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        if configured {
            info!("Device configured, it may now draw up to the configured current limit from Vbus.")
        } else {
            info!("Device is no longer configured, the Vbus current limit is 100mA.");
        }
    }
}