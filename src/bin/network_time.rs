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

use core::net::{IpAddr, SocketAddr};
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_net::udp::UdpSocket;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::timer::timg::TimerGroup;
use esp_println::println;
use esp_radio::Controller;
use esp_radio::wifi::{ClientConfig, ModeConfig, ScanConfig, WifiController, WifiDevice, WifiEvent, WifiStaState};
use log::{error, info, warn};
use sntpc::{get_time, NtpContext, NtpTimestampGenerator};
use esp_alloc as _;
use esp_backtrace as _;

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

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let rtc = Rtc::new(peripherals.LPWR);

    esp_alloc::heap_allocator!(size: 72 * 1024);
    let timer_g0 = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer_g0.timer0);

    info!("made timer");
    // let esp_radio_ctrl = esp_radio::init().unwrap();
    let esp_radio_ctrl = &*mk_static!(Controller<'static>, esp_radio::init().unwrap());

    let (controller, interfaces) =
        esp_radio::wifi::new(&esp_radio_ctrl, peripherals.WIFI, Default::default()).unwrap();

    let rng: Rng = Rng::new();
    let config = embassy_net::Config::dhcpv4(Default::default());
    let net_seed = (rng.random() as u64) << 32 | rng.random() as u64;
    info!("made net seed {}", net_seed);
    let tls_seed = rng.random() as u64 | ((rng.random() as u64) << 32);
    info!("made tls seed {}", tls_seed);

    info!("init-ing the network stack");
    // Init network stack
    let (stack, wifi_runner) = embassy_net::new(
        interfaces.sta,
        config,
        mk_static!(StackResources<3>, StackResources::<3>::new()),
        net_seed,
    );

    info!("spawning connection");
    spawner.spawn(connection(controller)).ok();
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
        match esp_radio::wifi::sta_state() {
            WifiStaState::Connected => {
                // wait until we're no longer connected
                info!("waiting to be disconnected");
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        // info!("wifi state is {:?}", esp_radio::wifi::wifi_state());
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
            let client_config = ModeConfig::Client(ClientConfig::default()
                .with_ssid(SSID.unwrap().into())
                .with_password(PASSWORD.unwrap().into())
                );
            controller.set_config(&client_config).unwrap();
            info!("Starting wifi");
            // initializing stack
            controller.start_async().await.unwrap();
            info!("Wifi started!");
        }
        info!("Scan");
        // scan for longer and show hidden
        let scan_config = ScanConfig::default().with_max(10);
        let mut result = controller
            .scan_with_config_async(scan_config)
            .await
            .unwrap();
        // sort by best signal strength first
        result.sort_by(|a, b| a.signal_strength.cmp(&b.signal_strength));
        result.reverse();
        for ap in result.iter() {
            info!("found AP: {:?}", ap);
        }
        info!("About to connect");
        match controller.connect_async().await {
            Ok(_) => info!("Wifi connected!"),
            Err(e) => {
                info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
        Timer::after(Duration::from_millis(1000)).await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
