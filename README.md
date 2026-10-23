# dht11-trace

DHT11 driver for [`embedded-hal`](https://crates.io/crates/embedded-hal) 1.0 that tells
where a read failed.

Most DHT11 drivers report a failed read as a timeout or a checksum error. `dht11-trace`
names the stage of the protocol that went wrong, which separates a wiring fault from a stuck
sensor or from a read that was interrupted halfway:

| `Fault` | What happened |
| --- | --- |
| `LineLow` | The line was already low before the start pulse: DATA shorted to ground or the sensor stuck |
| `NoAck` | No answer to the start pulse: no power, DATA disconnected or the sensor stuck |
| `AckBroken` | The answer started but did not follow the 80 µs low and 80 µs high of the datasheet |
| `Truncated { bits }` | The frame stopped after `bits` of its 40 bits |
| `Checksum { frame }` | The frame arrived complete, but the checksum does not match; the five bytes come with it |
| `Pin(error)` | The pin returned an error |

`Fault` implements `Display`, so a log line such as `DHT11: frame cut after 39 of 40 bits`
takes one `format!`. The crate is `no_std`, does not allocate and has no `unsafe` code.

## Usage

```rust
use dht11_trace::{Dht11, Platform};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};

fn report<P, D, T>(pin: P, delay: D, platform: T) -> Result<(), P::Error>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
{
    let mut dht11 = Dht11::new(pin, delay, platform)?;
    match dht11.read() {
        Ok(reading) => println!("{} °C, {} %", reading.temperature(), reading.humidity()),
        Err(fault) => println!("DHT11: {fault}"),
    }
    Ok(())
}
```

- **Pin:** open drain with a pull-up, and reading it must return the level of the line while
  the pin is released. DHT11 modules usually carry the pull-up; a bare sensor needs one of
  4.7–10 kΩ to VCC.
- **Timing rules:** wait 1 s after power-up before the first read, and at least 1 s between
  reads. The driver does not keep track of either.
- **Reading:** `humidity()` and `temperature()` return the integral parts; `bytes()` returns
  the four data bytes as received, decimal bytes included.

## The platform

A read blocks for about 23 ms: an 18 ms start pulse, then a 4 ms frame whose bits differ by
the length of a pulse, 26–28 µs for a 0 and 70 µs for a 1. Telling them apart takes two
things that `embedded-hal` 1.0 does not define, and the `Platform` trait asks for them:

- `ticks()` and `ticks_per_us()`: a free-running counter that keeps counting with interrupts
  masked, such as the CPU cycle counter. One tick per microsecond is enough.
- `uninterrupted(read)`: runs the frame read without interruption. An interrupt or a task
  switch of a few tens of microseconds corrupts the frame.

A `critical-section` implementation is not always enough for `uninterrupted`, because some
only exclude other critical sections. The one in `esp-idf-hal` is a FreeRTOS mutex and lets
the Wi-Fi task preempt the read. On the ESP32 with ESP-IDF, the cycle counter and an
`IsrCriticalSection` do the job:

```rust,ignore
use dht11_trace::Platform;
use esp_idf_svc::hal::interrupt::IsrCriticalSection;
use esp_idf_svc::sys;

struct Esp32 {
    section: IsrCriticalSection,
}

impl Platform for Esp32 {
    fn ticks(&self) -> u32 {
        unsafe { sys::xthal_get_ccount() }
    }

    fn ticks_per_us(&self) -> u32 {
        unsafe { sys::esp_rom_get_cpu_ticks_per_us() }
    }

    fn uninterrupted<R>(&self, read: impl FnOnce() -> R) -> R {
        let _guard = self.section.enter();
        read()
    }
}
```

[`examples/esp-idf`](examples/esp-idf) is a complete program for the ESP32.

## Timing

| Step | Value | Reason |
| --- | --- | --- |
| Start pulse | 18 ms low | Datasheet minimum |
| Acknowledgement check | 48 µs after release | Inside the 80 µs low answer, which starts 20–40 µs after release |
| Edge timeout | 100 µs | The longest phase of the protocol lasts 80 µs |
| 0 or 1 | High pulse longer than 48 µs is a 1 | 0 bits last 26–28 µs, 1 bits 70 µs |

The acknowledgement is checked at a fixed instant, instead of waiting for the line to rise
after release: with a weak pull-up, the line may not reach the high level before the sensor
pulls it low, and the frame would come out shifted by one bit.

## Testing

`cargo test` runs the driver against a simulated line, with no hardware: valid frames, every
fault, a weak pull-up, bits away from the datasheet lengths, a 1 MHz counter and a counter
that wraps around during the frame.

## Origin

The driver started in the reference firmware of
[SenseBridge](https://github.com/msc-sed-ufrr/SenseBridge), where it reads a DHT11 on an
ESP32 next to Wi-Fi traffic.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
