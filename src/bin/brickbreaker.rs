#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]

use core::default::Default;
use alloc::vec;
use alloc::vec::Vec;

use embedded_graphics::*;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::*;
// use embedded_graphics::geometry::{OriginDimensions, Point, Size};
// use embedded_graphics::mono_font::ascii::FONT_6X10;
// use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
// use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
// use embedded_graphics::text::Text;
use esp_hal::clock::CpuClock;
use esp_hal::{main, Config};
use log::info;
use rust_tdeck_experiments::Wrapper;

extern crate alloc;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

// This creates a default app-descriptor required by the esp-idf bootloader.
// For more information see: <https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/system/app_image_format.html#application-description>
esp_bootloader_esp_idf::esp_app_desc!();


pub struct Brick {
    pub bounds: Rectangle,
    pub color: Rgb565,
    pub active: bool,
}
pub struct GameView {
    pub bounds: Rectangle,
    pub paddle: Rectangle,
    pub old_paddle: Rectangle,
    pub visible: bool,
    pub count: i32,
    pub ball_bounds: Rectangle,
    pub ball_velocity: Point,
    pub bricks: Vec<Brick>,
}
impl GameView {
    pub fn new() -> Self {
        let colors = [Rgb565::GREEN, Rgb565::YELLOW, Rgb565::CSS_ORANGE, Rgb565::RED];
        let mut bricks:Vec<Brick> = Vec::new();
        for i in 0..6 {
            for j in 0..4 {
                bricks.push(Brick {
                    bounds: Rectangle::new(Point::new(40 + i * 40 as i32, 20+ j * 20 as i32), Size::new(35, 15)),
                    color: colors[(j as usize) % colors.len()],
                    active: true,
                })
            }
        }
        GameView {
            bounds: Rectangle::new(Point::new(0, 0), Size::new(200, 200)),
            paddle: Rectangle::new(Point::new(100, 220), Size::new(50, 10)),
            old_paddle: Rectangle::new(Point::new(100, 220), Size::new(50, 10)),
            visible: true,
            count: 0,
            ball_bounds: Rectangle::new(Point::new(100, 120), Size::new(10, 10)),
            ball_velocity: Point::new(2, 1),
            bricks,
        }
    }
}

impl GameView {
    pub(crate) fn handle_collisions(&mut self) {
        let old_ball_bounds = self.ball_bounds.clone();
        self.ball_bounds = self.ball_bounds.translate(self.ball_velocity);

        // collide with bricks
        for brick in &mut self.bricks {
            if brick.active && !self.ball_bounds.intersection(&brick.bounds).is_zero_sized() {
                brick.active = false;
                // from the bottom
                if old_ball_bounds.top_left.y > brick.bounds.top_left.y + brick.bounds.size.height as i32 {
                    info!("from the bottom");
                    self.ball_velocity.y = -self.ball_velocity.y
                }
                // from the top
                if (old_ball_bounds.top_left.y + old_ball_bounds.size.height as i32) < brick.bounds.top_left.y {
                    info!("from the top");
                    self.ball_velocity.y = -self.ball_velocity.y
                }
                // from the right
                if old_ball_bounds.top_left.x > brick.bounds.top_left.x + brick.bounds.size.width as i32 {
                    info!("from the right");
                    self.ball_velocity.x = -self.ball_velocity.x
                }
                // from the left
                if (old_ball_bounds.top_left.x + old_ball_bounds.size.width as i32) < brick.bounds.top_left.x {
                    info!("from the left");
                    self.ball_velocity.x = -self.ball_velocity.x
                }
            }
        }

        // collide with the screen edges
        if self.ball_bounds.top_left.y >= 240 - 20 {
            self.ball_velocity = Point::new(self.ball_velocity.x, -self.ball_velocity.y);
        }
        if self.ball_bounds.top_left.y <= 0 {
            self.ball_velocity = Point::new(self.ball_velocity.x, -self.ball_velocity.y);
        }
        if self.ball_bounds.top_left.x >= 320 - 20 {
            self.ball_velocity = Point::new(-self.ball_velocity.x, self.ball_velocity.y);
        }
        if self.ball_bounds.top_left.x <= 0 {
            self.ball_velocity = Point::new(-self.ball_velocity.x, self.ball_velocity.y);
        }

        // collide with the paddle
        let inter = self.ball_bounds.intersection(&self.paddle);
        if !inter.is_zero_sized() {
            self.ball_velocity = Point::new(self.ball_velocity.x, -self.ball_velocity.y);
        }

    }
}




#[main]
fn main() -> ! {
    esp_println::logger::init_logger_from_env();
    let config = Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let mut wrapper = Wrapper::init(peripherals);

    esp_alloc::heap_allocator!(size: 72 * 1024);

    info!("running");

    let mut game = GameView::new();

    loop {
        info!("Hello world!");

        wrapper.poll_keyboard();
        let color = Rgb565::RED;
        wrapper.display.clear(color).unwrap();
        let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);
        Text::new("Hello Rust!", Point::new(20, 30), style)
            .draw(&mut wrapper.display)
            .unwrap();

        game.draw(&mut wrapper);
        info!("battery is {}", wrapper.read_battery_level());
        wrapper.delay.delay_millis(100);
    }
}

impl GameView {
    fn draw(&mut self, context:&mut Wrapper) {
        self.count = self.count + 1;

        let old_ball_bounds = self.ball_bounds;
        self.handle_collisions();

        // draw background
        if self.count < 10 {
            let screen = Rectangle::new(Point::new(0, 0), Size::new(320, 240));
            screen.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK)).draw(&mut context.display).unwrap();
        }

        for brick in &self.bricks {
            if brick.active {
                brick.bounds.into_styled(PrimitiveStyle::with_fill(brick.color)).draw(&mut context.display).unwrap();
            } else {
                brick.bounds.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK)).draw(&mut context.display).unwrap();
            }
        }


        // draw the ball
        old_ball_bounds.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK)).draw(&mut context.display).unwrap();
        self.ball_bounds.into_styled(PrimitiveStyle::with_fill(Rgb565::MAGENTA)).draw(&mut context.display).unwrap();

        // draw the paddle
        self.old_paddle.into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK)).draw(&mut context.display).unwrap();
        self.paddle.into_styled(PrimitiveStyle::with_fill(Rgb565::RED)).draw(&mut context.display).unwrap();
    }

    fn handle_input(&mut self, event:&mut Wrapper) {
        self.old_paddle = self.paddle;
        let mut x = 0;
        if event.trackball_left {
            x = -1;
        }
        if event.trackball_right {
            x = 1;
        }
        self.paddle = self.paddle.translate(Point::new(x * 20, 0));
        if self.paddle.top_left.x < 0 {
            self.paddle.top_left.x = 0;
        }
        if self.paddle.top_left.x + (self.paddle.size.width as i32) > 320 {
            self.paddle.top_left.x = 320 - self.paddle.size.width as i32;
        }
        self.paddle.top_left.y = 200;
    }
}
