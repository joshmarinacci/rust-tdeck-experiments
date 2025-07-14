#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]


use alloc::string::{String, ToString};
use alloc::{format, vec};
use alloc::vec::Vec;
use core::cmp::max;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::main;
use esp_hal::spi::{ master::{Spi, Config as SpiConfig } };
use esp_hal::time::{Duration, Instant, Rate};
use embedded_hal_bus::spi::ExclusiveDevice;
use log::info;


use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    text::Text,
    mono_font::{ ascii::FONT_6X10, MonoTextStyle}
};
use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_7X13, FONT_7X13_BOLD, FONT_8X13, FONT_8X13_BOLD, FONT_8X13_ITALIC, FONT_9X15, FONT_9X15_BOLD};
use embedded_graphics::mono_font::iso_8859_14::FONT_6X12;
use embedded_graphics::mono_font::iso_8859_4::FONT_6X9;
use embedded_graphics::mono_font::iso_8859_9::FONT_7X14;
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use mipidsi::{models::ST7789, Builder};
use mipidsi::interface::SpiInterface;
use mipidsi::options::{ColorInversion, ColorOrder, Orientation, Rotation};
use LineStyle::{Header, Link};
use crate::LineStyle::Plain;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

pub const LILYGO_KB_I2C_ADDRESS: u8 =     0x55;

extern crate alloc;

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();

enum LineStyle {
    Header,
    Plain,
    Link,
}
struct TextRun {
    style: LineStyle,
    text: String,
}

impl TextRun {
    fn header(p0: &str) -> TextRun {
        TextRun {
            style: Header,
            text: p0.to_string(),
        }
    }
}

impl TextRun {
    fn plain(p0: &str) -> TextRun {
        TextRun {
            style: Plain,
            text: p0.to_string(),
        }
    }
}

struct TextLine {
    runs: Vec<TextRun>,
}

impl TextLine {
    fn with(runs: Vec<TextRun>) -> TextLine {
        TextLine {
            runs: Vec::from(runs),
        }
    }
}

impl TextLine {
    fn new(p0: &str) -> TextLine {
        TextLine {
            runs: Vec::from([
                TextRun::plain(p0),
            ])
        }
    }
}

#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let mut delay = Delay::new();

    // have to turn on the board and wait 500ms before using the keyboard
    let mut board_power = Output::new(peripherals.GPIO10, High, OutputConfig::default());
    board_power.set_high();
    delay.delay_millis(1000);


    // ==== display setup ====
    // https://github.com/Xinyuan-LilyGO/T-Deck/blob/master/examples/HelloWorld/HelloWorld.ino

    // set TFT CS to high
    let mut tft_cs = Output::new(peripherals.GPIO12, High, OutputConfig::default());
    tft_cs.set_high();
    let tft_miso = Input::new(peripherals.GPIO38, InputConfig::default().with_pull(Pull::Up));
    let tft_sck = peripherals.GPIO40;
    let tft_mosi = peripherals.GPIO41;
    let tft_dc = Output::new(peripherals.GPIO11, Low, OutputConfig::default());
    let mut tft_enable = Output::new(peripherals.GPIO42, High, OutputConfig::default());
    tft_enable.set_high();

    info!("creating spi device");
    let spi = Spi::new(peripherals.SPI2, SpiConfig::default()
        .with_frequency(Rate::from_mhz(40))
                       // .with_mode(Mode::_0)
    ).unwrap()
        .with_sck(tft_sck)
        .with_miso(tft_miso)
        .with_mosi(tft_mosi)
        ;
    let mut buffer = [0u8; 512];

    info!("setting up the display");
    let spi_delay = Delay::new();
    let spi_device = ExclusiveDevice::new(spi, tft_cs, spi_delay).unwrap();
    let di = SpiInterface::new(spi_device, tft_dc, &mut buffer);
    info!("building");
    let mut display = Builder::new(ST7789,di)
        // .reset_pin(tft_enable)
        .display_size(240,320)
        .invert_colors(ColorInversion::Inverted)
        .color_order(ColorOrder::Rgb)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        // .display_size(320,240)
        .init(&mut delay).unwrap();

    info!("initialized display");
    // wait for everything to boot up
    // delay.delay_millis(500);


    let mut i2c = I2c::new(
        peripherals.I2C0,
        Config::default().with_frequency(Rate::from_khz(100)).with_timeout(BusTimeout::Disabled),
    )
        .unwrap()
        .with_sda(peripherals.GPIO18)
        .with_scl(peripherals.GPIO8);
    info!("initialized I2C keyboard");

    let font = FONT_9X15;
    let line_height = font.character_size.height as i32;
    let char_width = font.character_size.width as i32;

    let bg_style = Rgb565::BLACK;
    let plain_style = MonoTextStyle::new(&font, Rgb565::WHITE);
    let header_style = MonoTextStyle::new(&FONT_9X15_BOLD, Rgb565::BLUE);
    let link_style = MonoTextStyle::new(&font, Rgb565::RED);
    let debug_style = MonoTextStyle::new(&font, Rgb565::CSS_ORANGE);
    let mut scroll_offset = 0;

    let viewport_height = 320/(10+4);
    info!("viewport height is {}", viewport_height);
    // setup lines
    let lines = vec![
        TextLine::with(vec![TextRun::header("Header Text")]),
        TextLine::new("  "),
        TextLine::new("This is some text "),
        TextLine::new("This is some more text "),
        TextLine::with(vec![
            TextRun{
                style:Plain,
                text: "This is some text with ".to_string(),
            },
            TextRun{
                style:Link,
                text: "a link".to_string(),
            },
            TextRun{
                style:Plain,
                text: " inside it".to_string(),
            },
        ])
    ];



    let x_inset = 5;
    let y_offset = 10;
    let mut dirty = true;
    loop {
        if (dirty) {
            dirty = false;
            // clear display
            display.clear(bg_style).unwrap();
            info!("drawing lines at scroll {}", scroll_offset);
            // select the lines in the current viewport
            // draw the lines
            for (j, line) in lines.iter().enumerate() {
                let mut inset: usize = 0;
                let y = (j + scroll_offset) as i32 * line_height + y_offset;
                Text::new(&format!("{}", j), Point::new(x_inset, y), debug_style).draw(&mut display).unwrap();
                for (i, run) in line.runs.iter().enumerate() {
                    let pos = Point::new(x_inset + 15 + (inset as i32) * char_width, y);
                    let style = match run.style {
                        Plain => plain_style,
                        Header => header_style,
                        Link => link_style,
                    };
                    Text::new(&run.text, pos, style).draw(&mut display).unwrap();
                    inset += run.text.len();
                }
            }
        }

        // wait for up and down actions
        let mut data = [0u8; 1];
        let kb_res = i2c.read(LILYGO_KB_I2C_ADDRESS, &mut data);
        match kb_res {
            Ok(_) => {
                if data[0] != 0x00 {
                    info!("kb_res = {:?}", String::from_utf8_lossy(&data));
                    // scroll up and down
                    if data[0] == b'k' {
                        scroll_offset += 1;
                        dirty = true;
                    }
                    if data[0] == b'j' {
                        scroll_offset = if scroll_offset > 0 { scroll_offset - 1 } else { 0 };
                        dirty = true;
                    }
                }
            },
            Err(e) => {
                // info!("kb_res = {}", e);
            }
        }
        delay.delay_millis(100);
    }
}

