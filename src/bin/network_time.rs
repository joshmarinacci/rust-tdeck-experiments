//! Embassy SNTP example
//!
//!
//! Set SSID and PASSWORD env variable before running this example.
//!
//! This gets an ip address via DHCP then performs an SNTP request to update the RTC time with the
//! response. The RTC time is then compared with the received data parsed with jiff.
//! You can change the timezone to your local timezone.
// copied from https://github.com/esp-rs/esp-hal/blob/main/examples/wifi/sntp/src/main.rs
#![no_std]
#![no_main]
extern crate alloc;

use alloc::format;
use alloc::string::ToString;
use core::net::{IpAddr, SocketAddr};

use embassy_executor::Spawner;
use embassy_net::{
    Runner,
    Stack,
    StackResources,
    dns::DnsQueryType,
    udp::{PacketMetadata, UdpSocket},
};
use embassy_time::{Duration, Timer};
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rng::Rng, rtc_cntl::Rtc, timer::timg::TimerGroup};
use esp_println::println;
use esp_wifi::{wifi::{ClientConfiguration, Configuration, ScanConfig, WifiController, WifiDevice, WifiEvent}, EspWifiController};
use esp_wifi::wifi::ScanTypeConfig::Active;
use esp_wifi::wifi::WifiState;
use log::{error, info, warn};
use sntpc::{NtpContext, NtpTimestampGenerator, get_time};

esp_bootloader_esp_idf::esp_app_desc!();

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: Option<&str> = option_env!("SSID");
const PASSWORD: Option<&str> = option_env!("PASSWORD");
const TIMEZONE: jiff::tz::TimeZone = jiff::tz::get!("UTC");
const NTP_SERVER: &str = "pool.ntp.org";

/// Microseconds in a second
const USEC_IN_SEC: u64 = 1_000_000;

#[derive(Clone, Copy)]
struct Timestamp<'a> {
    rtc: &'a Rtc<'a>,
    current_time_us: u64,
}

impl NtpTimestampGenerator for Timestamp<'_> {
    fn init(&mut self) {
        self.current_time_us = self.rtc.current_time_us();
    }

    fn timestamp_sec(&self) -> u64 {
        self.current_time_us / 1_000_000
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.current_time_us % 1_000_000) as u32
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let rtc = Rtc::new(peripherals.LPWR);

    esp_alloc::heap_allocator!(size: 72 * 1024);
    let mut rng = Rng::new(peripherals.RNG);
    let timer_g0 = TimerGroup::new(peripherals.TIMG0);

    info!("made timer");
    // let esp_wifi_ctrl = esp_wifi::init(timer_g0.timer0, rng.clone()).unwrap();
    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer_g0.timer0, rng.clone()).unwrap()
    );
    info!("making controller");
    let (wifi_controller, interfaces) =
        esp_wifi::wifi::new(&esp_wifi_ctrl, peripherals.WIFI).unwrap();
    let wifi_interface = interfaces.sta;

    let config = embassy_net::Config::dhcpv4(Default::default());
    let net_seed = (rng.random() as u64) << 32 | rng.random() as u64;
    info!("made net seed {}", net_seed);
    let tls_seed = rng.random() as u64 | ((rng.random() as u64) << 32);
    info!("made tls seed {}", tls_seed);

    info!("init-ing the network stack");
    // Init network stack
    let (stack, wifi_runner) = embassy_net::new(
        wifi_interface,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    info!("spawning connection");
    spawner.spawn(connection(wifi_controller)).ok();
    info!("spawning net task");
    spawner.spawn(net_task(wifi_runner)).ok();

    wait_for_connection(stack).await;

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    let ntp_addrs = stack.dns_query(NTP_SERVER, DnsQueryType::A).await.unwrap();

    if ntp_addrs.is_empty() {
        panic!("Failed to resolve DNS. Empty result");
    }

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );

    socket.bind(123).unwrap();

    // Display initial Rtc time before synchronization
    let now = jiff::Timestamp::from_microsecond(rtc.current_time_us() as i64).unwrap();
    info!("Rtc: {now}");

    loop {
        let addr: IpAddr = ntp_addrs[0].into();
        let result = get_time(
            SocketAddr::from((addr, 123)),
            &socket,
            NtpContext::new(Timestamp {
                rtc: &rtc,
                current_time_us: 0,
            }),
        )
            .await;

        match result {
            Ok(time) => {
                // Set time immediately after receiving to reduce time offset.
                rtc.set_current_time_us(
                    (time.sec() as u64 * USEC_IN_SEC)
                        + ((time.sec_fraction() as u64 * USEC_IN_SEC) >> 32),
                );

                // Compare RTC to parsed time
                info!(
                    "Response: {:?}\nTime: {}\nRtc : {}",
                    time,
                    // Create a Jiff Timestamp from seconds and nanoseconds
                    jiff::Timestamp::from_second(time.sec() as i64)
                        .unwrap()
                        .checked_add(
                            jiff::Span::new()
                                .nanoseconds((time.seconds_fraction as i64 * 1_000_000_000) >> 32),
                        )
                        .unwrap()
                        .to_zoned(TIMEZONE),
                    jiff::Timestamp::from_microsecond(rtc.current_time_us() as i64)
                        .unwrap()
                        .to_zoned(TIMEZONE)
                );
            }
            Err(e) => {
                error!("Error getting time: {e:?}");
            }
        }

        Timer::after(Duration::from_secs(10)).await;
    }
}


async fn wait_for_connection(stack: Stack<'_>) {
    info!("Waiting for link to be up");
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    info!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            info!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    info!("start connection task");
    info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                info!("waiting to be disconnected");
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        info!("wifi state is {:?}", esp_wifi::wifi::wifi_state());
        // DISCONNECTED
        info!(
            "we are disconnected. is started = {:?}",
            controller.is_started()
        );
        if !matches!(controller.is_started(), Ok(true)) {
            if SSID.is_none() {
                warn!("SSID is none. did you forget to set the SSID environment variables");
            }
            if PASSWORD.is_none() {
                warn!("PASSWORD is none. did you forget to set the PASSWORD environment variables");
            }
            let client_config = Configuration::Client(ClientConfiguration {
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            info!("Starting wifi");
            // initializing stack
            controller.start_async().await.unwrap();
            info!("Wifi started!");
        }
        info!("Scan");
        // scan for longer and show hidden
        let active = Active {
            min: core::time::Duration::from_millis(50),
            max: core::time::Duration::from_millis(100),
        };
        // scanning
        let mut result = controller
            .scan_with_config_async(ScanConfig {
                show_hidden: true,
                scan_type: active,
                ..Default::default()
            })
            .await
            .unwrap();
        // sort by best signal strength first
        result.sort_by(|a, b| a.signal_strength.cmp(&b.signal_strength));
        result.reverse();
        // for ap in result.iter() {
        //     // info!("found AP: {:?}", ap);
        // }
        // pick the first that matches the passed in SSID
        let ap = result
            .iter()
            .filter(|ap| ap.ssid.eq_ignore_ascii_case(SSID.unwrap()))
            .next();
        if let Some(ap) = ap {
            info!("using the AP {:?}", ap);
            // set the config to use for connecting
            controller
                .set_configuration(&Configuration::Client(ClientConfiguration {
                    ssid: ap.ssid.to_string(),
                    password: PASSWORD.unwrap().into(),
                    ..Default::default()
                }))
                .unwrap();

            info!("About to connect");
            match controller.connect_async().await {
                Ok(_) => {
                    info!("Wifi connected!");
                    loop {
                        info!("checking if we are still connected");
                        if let Ok(conn) = controller.is_connected() {
                            if conn {
                                info!("Connected successfully");
                                info!("sleep until we aren't connected anymore");
                                Timer::after(Duration::from_millis(5000)).await
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to connect to wifi: {e:?}");
                    Timer::after(Duration::from_millis(5000)).await
                }
            }
        } else {
            let ssid = SSID.unwrap();
            info!("did not find the ap for {ssid}");
            info!("looping around");
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
