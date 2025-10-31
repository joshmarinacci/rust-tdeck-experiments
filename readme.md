This repo has examples of embedded Rust (no_std) using the [esp_hal](https://github.com/esp-rs/esp-hal) for 
the **Lilygo T-deck**.

# What is this?

The [Lilygo T-Deck](https://lilygo.cc/products/t-deck?srsltid=AfmBOopOffpUkKoRUwHZOJjJLkhJ82Lr_EBmVzEzuBQSIlQjM8_idPMr) is
a small embedded device with a screen and tiny keyboard built on an ESP32 processor, including Wi-Fi.

You can officially program the T-Deck using Arduino and Micro/CircuitPython, but I wanted to get Rust running on it. 
Embedded Rust usually uses no_std, meaning the standard APIs are not available. Instead you use special embedded apis provided
by the [esp_hal](https://github.com/esp-rs/esp-hal) project. 

The examples here show using esp_hal with the T-Deck to access the wifi chip, scan the keyboard, display graphics on
the screen, and access the trackball.

# Examples:

* [hello](src/bin/hello.rs) Just prints hello world to the terminal. Use this to make sure your toolchain is up and running correctly.
* [audio_wavforms](src/bin/audio_wavforms.rs) Generates and plays a sawtooth waveform to the speaker.
* [battery](src/bin/battery.rs) Reads the current battery level from an analog pin.
* [backlight](src/bin/backlight.rs) **New!** Cycles the display backlight from 0 to 100% using PWM.
* [brickbreaker](src/bin/brickbreaker.rs) **New!** A simple brick breaking game using the trackball.
* [display](src/bin/display.rs) Draws text and background colors to the screen
* [flash](src/bin/flash.rs) **New!** Print size of internal flash and lists partitions in the partition table.
* [info](src/bin/info.rs) Shows how to get info on the board including the chip name, free memory, and the MAC address.
* [keyboard](src/bin/keyboard.rs). Poll the keyboard for keystrokes over the I2C bus.
* [network_time](src/bin/network_time.rs). **New!** Use NTP to get the network time over wi-fi.
* [sdcard](src/bin/sdcard.rs) List files from the SD card. **NOTE** Requires and SD card formatted with FAT/MSFAT. ExtFat doesn't seem to work.
* [term](src/bin/term.rs). Prints the typed text to the screen.
* [touch](src/bin/touch.rs). Polls for events from the touch screen. 
* [trackball](src/bin/trackball.rs). Polls the trackball for motion events and clicks.
* [wifi_scan](src/bin/wifi_scan.rs). Turns on the wifi chip, scans for access points, then makes a simple HTTP request.
* [wrapper](src/bin/wrapper.rs). **New!** Uses a wrapper struct to make working with the T-Deck hardware easier.

# How to Run them

Assuming you already have the toolchains installed, just run `cargo run --bin <example_name>`. Ex:

```shell
cargo run --bin info
```

For the network examples you'll need to specify the SSID and PASSWORD in the code or on the command line.

```shell
SSID=MyCoolNetwork PASSWORD=BestPasswordEvar run --bin wifi_scan 
```

# What Versions?

The esp_hal project recently started focusing on stability and moving towards a 1.0 release.  These examples
work on the latest version of esp_hal, which as of the time of this writing (October 2025) is
[v1.0.0](https://github.com/esp-rs/esp-hal/releases/tag/esp-hal-v1.0.0). I will do my best
to keep them up to date.

# Why?

I love messing around with CircuitPython, but for projects that need faster processing and more robust code I need
a stronger language. I like statically typed languages and refuse to code C++, so that makes Rust
the best option. Rust embedded is not as mature as Rust for server/desktop, but you can still do a lot. The biggest
challenge is that the embedded APIs change frequently. Example code I found from two years ago won't compile anymore.
So after I finally got my own code to compile I figured I'd share it here. Please do whatever you want
with the code and let me know if it helps you.

# What's next

* Audio
* SD Card
* Lora
* BT / BTLE
 
I really want to get the audio working which involves understanding how I2S works. Then, combined
with SD card support, I can build my own MP3 player! Any help would be greatly appreciated.

# Thanks

Special thanks to GitHub user *tstellanova* for [their example code from two years ago](https://github.com/tstellanova/tweedeck).


