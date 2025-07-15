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
use core::ops::Add;
use esp_hal::clock::CpuClock;
use esp_hal::gpio::{Input, InputConfig, Output, OutputConfig, Pull};
use esp_hal::delay::Delay;
use esp_hal::gpio::Level::{High, Low};
use esp_hal::{main, Blocking};
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
use embedded_graphics::mono_font::MonoFont;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use esp_hal::i2c::master::{BusTimeout, Config, I2c};
use mipidsi::{models::ST7789, Builder, Display, NoResetPin};
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

#[derive(Clone, Copy)]
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

struct MenuView<'a> {
    id: &'a str,
    items: Vec<&'a str>,
    position: Point,
    highlighted_index: usize,
    visible: bool,
    dirty: bool,
}

impl<'a> MenuView<'a> {
    pub(crate) fn is_visible(&self) -> bool {
        return self.visible;
    }
}

impl<'a> MenuView<'a> {
    pub(crate) fn show(&mut self) {
        self.visible = true;
        self.dirty = true;
    }
    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.dirty = true;
    }
    pub(crate) fn nav_prev(&mut self) {
        self.highlighted_index = (self.highlighted_index + 1) % self.items.len();
        self.dirty = true;
    }
    pub(crate) fn nav_next(&mut self) {
        self.highlighted_index = (self.highlighted_index + self.items.len() - 1) % self.items.len();
        self.dirty = true;
    }
}

impl MenuView<'_> {
    fn new<'a>(id:&'a str, items: &Vec<&'a str>, p1: Point) -> MenuView<'a> {
        MenuView {
            id:id,
            items:items.to_vec(),
            position:p1,
            highlighted_index: 0,
            visible: false,
            dirty: true
        }
    }
    fn draw(&mut self, display: &mut Display<SpiInterface<ExclusiveDevice<Spi<Blocking>, Output, Delay>, Output>, ST7789, NoResetPin>) {
        if !self.visible {
            return;
        }
        if !self.dirty {
            return;
        }
        let font = FONT_9X15;
        let lh = font.character_size.height as i32;
        let pad = 5;
        let rect = Rectangle::new(self.position, Size::new(100,(self.items.len() as i32 * lh + pad * 2) as u32));
        rect.into_styled(PrimitiveStyle::with_fill(Rgb565::CSS_LIGHT_GRAY)).draw(display).unwrap();
        // info!("Highlighted index {}", self.highlighted_index);
        for (i,item) in self.items.iter().enumerate() {
            let bg = if i == self.highlighted_index {
                Rgb565::RED
            } else {
                Rgb565::WHITE
            };
            let fg = if i == self.highlighted_index {
                Rgb565::WHITE
            } else {
                Rgb565::RED
            };
            let ly = (i as i32)*lh + pad;
            Rectangle::new(Point::new(pad,ly).add(self.position), Size::new(100, lh as u32))
                .into_styled(PrimitiveStyle::with_fill(bg)).draw(display).unwrap();
            let text_style = MonoTextStyle::new(&font, fg);
            Text::new(&item, Point::new(pad,ly+lh -2 ).add(self.position), text_style).draw(display).unwrap();
        }
        self.dirty = false;
    }
}

struct BrowserTheme<'a> {
    bg: Rgb565,
    font: &'a MonoFont<'a>,
    header: MonoTextStyle<'a, Rgb565>,
    plain: MonoTextStyle<'a, Rgb565>,
    link: MonoTextStyle<'a, Rgb565>,
    debug: MonoTextStyle<'a, Rgb565>,
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
    info!("heap is {}", esp_alloc::HEAP.stats());
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

    let max_chars = display.size().width / font.character_size.width;
    info!("display width in chars: {}", max_chars);

    let dark_theme = BrowserTheme {
        bg: Rgb565::BLACK,
        font: &FONT_9X15,
        header: MonoTextStyle::new(&FONT_9X15, Rgb565::RED),
        plain: MonoTextStyle::new(&FONT_9X15, Rgb565::WHITE),
        link: MonoTextStyle::new(&FONT_9X15, Rgb565::BLUE),
        debug: MonoTextStyle::new(&FONT_9X15, Rgb565::CSS_ORANGE),
    };
    let light_theme = BrowserTheme {
        bg: Rgb565::WHITE,
        font: &FONT_9X15,
        header: MonoTextStyle::new(&FONT_9X15, Rgb565::RED),
        plain: MonoTextStyle::new(&FONT_9X15, Rgb565::BLACK),
        link: MonoTextStyle::new(&FONT_9X15, Rgb565::BLUE),
        debug: MonoTextStyle::new(&FONT_9X15, Rgb565::CSS_ORANGE),
    };
    let mut theme = &dark_theme;
    let mut scroll_offset:i32 = 0;

    let viewport_height:i32 = (display.size().height / font.character_size.height) as i32;
    info!("viewport height is {} rows", viewport_height);
    let mut lines:Vec<TextLine> = vec![];
    lines.append(&mut break_lines("Thoughts on LLMs and the coming AI backlash", max_chars-4,LineStyle::Header));
    lines.push(TextLine {
        runs: vec![TextRun {
            style:Plain,
            text:"".into(),
        }]
    });
    lines.append(&mut break_lines(r#"I find Large Language Models fascinating.
    They are a very different approach to AI than most of the 60 years of
    AI research and show great promise. At the same time they are just technology.
    They aren't magic. They aren't even very good technology yet. LLM hype has vastly
    outpaced reality and I think we are due for a correction, possibly even a bubble pop.
    Furthermore, I think future AI progress is going to happen on the app / UX side,
    not on the core models, which are already starting to show their scaling limits.
    Let's dig in. Better pour a cup of coffee. This could be a long one."#, max_chars-4,LineStyle::Plain));



    let x_inset = 5;
    let mut dirty = true;
    let main_menu_items = vec![
        "Theme",
        "Font",
        "Wifi",
        "Close",
    ];
    let theme_menu_items = vec![
        "Dark",
        "Light",
        "Close",
    ];
    let font_menu_items = vec![
        "7x8",
        "8x12",
        "9x15",
        "Close",
    ];
    let wifi_menu_items = vec![
        "SSID1",
        "SSID2",
        "SSID3",
        "Close",
    ];
    let mut main_menu = MenuView::new("main",&main_menu_items, Point::new(0, 0));
    let mut theme_menu = MenuView::new("theme",&theme_menu_items, Point::new(50, 20));
    let mut font_menu = MenuView::new("font",&font_menu_items, Point::new(50, 20));
    let mut wifi_menu = MenuView::new("wifi",&wifi_menu_items, Point::new(50, 20));
    let mut target_menu = "main";
    loop {
        if (dirty) {
            dirty = false;
            // clear display
            display.clear(theme.bg).unwrap();
            info!("drawing lines at scroll {}", scroll_offset);
            // select the lines in the current viewport
            // draw the lines
            let mut end:usize = (scroll_offset + viewport_height) as usize;
            if end >= lines.len() {
                end = lines.len();
            }
            let viewport_lines = &lines[(scroll_offset as usize) .. end];
            for (j, line) in viewport_lines.iter().enumerate() {
                let mut inset_chars: usize = 3;
                let y = j as i32 * line_height + 10;
                Text::new(&format!("{}", j), Point::new(x_inset, y), theme.debug).draw(&mut display).unwrap();
                for (i, run) in line.runs.iter().enumerate() {
                    let pos = Point::new(inset_chars as i32 * char_width, y);
                    let style = match run.style {
                        Plain => theme.plain,
                        Header => theme.header,
                        Link => theme.link,
                    };
                    Text::new(&run.text, pos, style).draw(&mut display).unwrap();
                    inset_chars += run.text.len();
                }
            }
            // info!("heap is {}", esp_alloc::HEAP.stats());
        }
        main_menu.draw(&mut display);
        theme_menu.draw(&mut display);
        font_menu.draw(&mut display);
        wifi_menu.draw(&mut display);

        // wait for up and down actions
        let mut data = [0u8; 1];
        let kb_res = i2c.read(LILYGO_KB_I2C_ADDRESS, &mut data);
        match kb_res {
            Ok(_) => {
                if data[0] != 0x00 {
                    info!("kb_res = {:?}", String::from_utf8_lossy(&data));
                    // scroll up and down
                    if data[0] == b'j' {
                        if target_menu == "main" {
                            main_menu.nav_prev();
                        } else if target_menu == "theme" {
                            theme_menu.nav_prev();
                        } else if target_menu == "font" {
                            font_menu.nav_prev();
                        } else if target_menu == "wifi" {
                            wifi_menu.nav_prev();
                        } else {
                            if scroll_offset + viewport_height < lines.len() as i32 {
                                scroll_offset = scroll_offset + viewport_height;
                            }
                            dirty = true;
                        }
                    }
                    if data[0] == b' ' {
                        if target_menu == "main" {
                            if main_menu.is_visible() {
                                main_menu.hide();
                            } else {
                                main_menu.show();
                            }
                        }
                        dirty = true;
                    }
                    if data[0] == b'k' {
                        if target_menu == "main" {
                            main_menu.nav_next();
                        } else if target_menu == "theme" {
                            theme_menu.nav_next();
                        } else if target_menu == "font" {
                            font_menu.nav_next();
                        } else if target_menu == "wifi" {
                            wifi_menu.nav_next();
                        } else {
                            scroll_offset = if (scroll_offset - viewport_height) >= 0 { scroll_offset - viewport_height } else { 0 };
                            info!("scroll_offset = {}", scroll_offset);
                            dirty = true;
                        }
                    }
                    if data[0] == b'\r' {
                        dirty = true;
                        if target_menu == main_menu.id {
                            let action = theme_menu.items[theme_menu.highlighted_index];
                            // info!("selected item {}",action);
                            if action == "Close" {
                                main_menu.hide();
                            }
                            if main_menu.highlighted_index == 0 {
                                theme_menu.show();
                                target_menu = theme_menu.id;
                                main_menu.dirty = true;
                            }
                            if main_menu.highlighted_index == 1 {
                                font_menu.show();
                                target_menu = font_menu.id;
                                main_menu.dirty = true;
                            }
                            if main_menu.highlighted_index == 2 {
                                wifi_menu.show();
                                target_menu = wifi_menu.id;
                                main_menu.dirty = true;
                            }
                        }
                        if target_menu == theme_menu.id {
                            let action = theme_menu.items[theme_menu.highlighted_index];
                            // info!("selected item {}",action);
                            if theme_menu.highlighted_index == 0 {
                                theme = &dark_theme;
                                dirty = true;
                                theme_menu.dirty = true;
                                main_menu.dirty = true;
                            }
                            if theme_menu.highlighted_index == 1 {
                                theme = &light_theme;
                                dirty = true;
                                theme_menu.dirty = true;
                                main_menu.dirty = true;
                            }
                            if action == "Close" {
                                theme_menu.hide();
                                target_menu = main_menu.id;
                                theme_menu.dirty = true;
                                main_menu.dirty = true;
                            }
                        }
                        if target_menu == font_menu.id {
                            let action = font_menu.items[font_menu.highlighted_index];
                            info!("selected item {}",action);
                            if action == "Close" {
                                font_menu.hide();
                                target_menu = main_menu.id;
                                main_menu.dirty = true;
                            }
                        }
                        if target_menu == wifi_menu.id {
                            let action = wifi_menu.items[wifi_menu.highlighted_index];
                            info!("selected item {}",action);
                            if action == "Close" {
                                wifi_menu.hide();
                                target_menu = main_menu.id;
                                main_menu.dirty = true;
                            }
                        }
                    }
                }
            },
            Err(e) => {
                // info!("kb_res = {}", e);
            }
        }
        // delay.delay_millis(100);
    }
}

fn break_lines(text: &str, width: u32, style: LineStyle) -> Vec<TextLine> {
    let mut lines: Vec<TextLine> = vec![];
    let mut tl:TextLine = TextLine {
        runs: vec![],
    };
    let mut bucket = String::new();
    for (i,word) in text.split(' ').enumerate() {
        let word = word.trim();
        // info!("word = {:?}", word);
        if word == "" {
            continue;
        }
        if bucket.len() + word.len() < width as usize {
            bucket.push_str(word);
            bucket.push_str(" ");
        } else {
            tl.runs.push(TextRun{
                style: style.clone(),
                text: bucket.clone(),
            });
            lines.push(tl);
            tl = TextLine {
                runs: vec![],
            };
            bucket.clear();
            bucket.push_str(word);
            bucket.push_str(" ");
        }
    }
    tl.runs.push(TextRun{
        style:style.clone(),
        text: bucket.clone(),
    });
    lines.push(tl);
    return lines;
}

