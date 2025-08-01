#![no_std]
#![no_main]

extern crate alloc;
use core::net::Ipv4Addr;

use blocking_network_stack::Stack;
use embedded_io::*;

use esp_alloc as _;

use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    gpio::Level::High,
    gpio::{Output, OutputConfig},
    main,
    rng::Rng,
    time::{Duration, Instant},
    timer::timg::TimerGroup
};
use esp_println::{println};
use esp_wifi;
use esp_wifi::wifi::{ClientConfiguration, Configuration, AccessPointInfo, WifiError};
use log::info;


use smoltcp::{
    iface::{SocketSet, SocketStorage},
    wire::{DhcpOption, IpAddress},
};


#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();


const SSID: &str = "JEFF22G";
const PASSWORD: &str = "Jefferson2022";

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    // turn on the board power and wait 1 second
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    let delay = Delay::new();
    delay.delay_millis(1000);

    info!("power is on");


    let mut rng = Rng::new(peripherals.RNG);

    // init the wifi chip
    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let esp_wifi_ctrl =  esp_wifi::init(timg0.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap();

    // access the wifi controller
    let (mut controller, interfaces) =
        esp_wifi::wifi::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();

    let mut device = interfaces.sta;
    let iface = create_interface(&mut device);


    // configure DHCP
    let mut socket_set_entries: [SocketStorage; 3] = Default::default();
    let mut socket_set = SocketSet::new(&mut socket_set_entries[..]);
    let mut dhcp_socket = smoltcp::socket::dhcpv4::Socket::new();
    // we can set a hostname here (or add other DHCP options)
    dhcp_socket.set_outgoing_options(&[DhcpOption {
        kind: 12,
        data: b"esp-wifi",
    }]);
    socket_set.add(dhcp_socket);

    info!("dhcp configured");


    let now = || Instant::now().duration_since_epoch().as_millis();
    let stack = Stack::new(iface, device, socket_set, now, rng.random());

    // disable power savings
    controller
        .set_power_saving(esp_wifi::config::PowerSaveMode::None)
        .unwrap();


    let client_config = Configuration::Client(ClientConfiguration {
        ssid: SSID.try_into().unwrap(),
        password: PASSWORD.try_into().unwrap(),
        ..Default::default()
    });
    let res = controller.set_configuration(&client_config);
    println!("wifi_set_configuration returned {:?}", res);

    controller.start().unwrap();
    println!("is wifi started: {:?}", controller.is_started());

    println!("Start Wifi Scan");
    let res = controller.scan_n(10); // 10 sec timeout?
    if let Ok((res)) = res {
        for ap in res {
            println!("{:?}", ap);
        }
    }

    println!("{:?}", controller.capabilities());
    println!("wifi_connect {:?}", controller.connect());

    // wait to get connected
    println!("Wait to get connected");
    loop {
        match controller.is_connected() {
            Ok(true) => break,
            Ok(false) => {}
            Err(err) => {
                println!("{:?}", err);
                loop {}
            }
        }
    }
    println!("controller connected = {:?}", controller.is_connected());

    // // wait for getting an ip address
    println!("Wait to get an ip address");
    loop {
        stack.work();

        if stack.is_iface_up() {
            println!("got ip {:?}", stack.get_ip_info());
            break;
        }
    }

    println!("Start busy loop on main");




    // make a simple HTTP request
    let mut rx_buffer = [0u8; 1536];
    let mut tx_buffer = [0u8; 1536];
    let mut socket = stack.get_socket(&mut rx_buffer, &mut tx_buffer);

    loop {
        println!("Making HTTP request");
        socket.work();

        socket
            .open(IpAddress::Ipv4(Ipv4Addr::new(142, 250, 185, 115)), 80)
            .unwrap();

        socket
            .write(b"GET / HTTP/1.0\r\nHost: www.mobile-j.de\r\n\r\n")
            .unwrap();
        socket.flush().unwrap();

        let deadline = Instant::now() + Duration::from_secs(20);
        let mut buffer = [0u8; 512];
        while let Ok(len) = socket.read(&mut buffer) {
            let to_print = unsafe { core::str::from_utf8_unchecked(&buffer[..len]) };
            println!("{}", to_print);

            if Instant::now() > deadline {
                println!("Timeout");
                break;
            }
        }
        println!();

        socket.disconnect();

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            socket.work();
        }
    }
}

fn timestamp() -> smoltcp::time::Instant {
    smoltcp::time::Instant::from_micros(
        esp_hal::time::Instant::now()
            .duration_since_epoch()
            .as_micros() as i64,
    )
}

pub fn create_interface(device: &mut esp_wifi::wifi::WifiDevice) -> smoltcp::iface::Interface {
    // users could create multiple instances but since they only have one WifiDevice
    // they probably can't do anything bad with that
    smoltcp::iface::Interface::new(
        smoltcp::iface::Config::new(smoltcp::wire::HardwareAddress::Ethernet(
            smoltcp::wire::EthernetAddress::from_bytes(&device.mac_address()),
        )),
        device,
        timestamp(),
    )
}
