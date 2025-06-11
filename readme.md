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

# What Versions?

The esp_hal project recently started focusing on stability and moving towards a 1.0 release.  These examples
work on the latest version of esp_hal, which as of the time of this writing (June 2025) is
[v1.0.0-beta.1](https://github.com/esp-rs/esp-hal/releases/tag/esp-hal-v1.0.0-beta.1). I will do my best
to keep them up to date.

# Why?

I love messing around with CircuitPython, but for projects that need faster processing and more robust code I need
a stronger language. I like statically typed languages and refuse to code C++, so that makes Rust
the best option. Rust embedded is not as mature as Rust for server/desktop, but you can still do a lot. The biggest
challenge is that the embedded APIs change frequently. Example code I found from two years ago won't compile anymore.
So after I finally got my own code to compile I figured I'd share it here. Please do whatever you want
with the code and let me know if it helps you.

# Thanks

Special thanks to GitHub user *tstellanova* for [their example code from two years ago](https://github.com/tstellanova/tweedeck).


