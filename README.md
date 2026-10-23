# dht11-trace

DHT11 driver for [`embedded-hal`](https://crates.io/crates/embedded-hal) 1.0 that tells
where a read failed.

Most DHT11 drivers report a failed read as a timeout or a checksum error. `dht11-trace`
names the stage of the protocol that went wrong, which separates a wiring fault from a stuck
sensor or from a read that was interrupted halfway:

| `Fault` | What happened |
| --- | --- |
| `LineLow` | The line was already low before the start pulse: DATA shorted to ground or the sensor stuck |
| `NoAck` | No answer to the start pulse: no power, DATA disconnected or the sensor stuck ([restart](#restarting-a-stuck-sensor)) |
| `AckBroken` | The answer started but did not follow the 80 µs low and 80 µs high of the datasheet |
| `Truncated { bits }` | The frame stopped after `bits` of its 40 bits |
| `Checksum { frame }` | The frame arrived complete, but the checksum does not match; the five bytes come with it |
| `ZeroFrame` | All 40 bits read as 0, which a working sensor never sends: the pulses were measured too short, usually because of a wrong `ticks_per_us` |
| `Pin(error)` | The pin returned an error |

`Fault` implements `Display`, so a log line such as `DHT11: frame cut after 39 of 40 bits`
takes one `format!`. It is `#[non_exhaustive]`: new stages may come in minor versions. The crate is `no_std`, does not allocate and has no `unsafe` code.

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
  the four data bytes as received. Some parts send tenths of a degree in the temperature's
  decimal byte, such as `[43, 0, 27, 6]` for 27.6 °C.

## Restarting a stuck sensor

A stuck DHT11 only answers again after losing power. With its VCC on a GPIO, the driver can
restart it: `Dht11::new(data, delay, platform)?.with_power(vcc)?`.

```rust
use dht11_trace::{Dht11, Platform, PowerError};
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};

fn read_and_recover<P, D, T, W>(
    dht11: &mut Dht11<P, D, T, W>,
) -> Result<(), PowerError<P::Error, W::Error>>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
    W: OutputPin,
{
    if let Err(fault) = dht11.read() {
        println!("DHT11: {fault}");
    }
    // Between reads, outside any time-critical work: restart after 3 failures in a row.
    if dht11.power_cycle_after(3)? {
        println!("DHT11 restarted");
    }
    Ok(())
}
```

- `power_cycle()` holds DATA low, cuts the power for 1 s, restores it and waits the 1 s
  power-up time, so it blocks for about 2 s. DATA stays low because the pull-up could
  otherwise keep the sensor powered through it.
- `failures()` counts failed reads in a row since the last good read or restart. Pin errors
  do not count: they come from the MCU, and restarting the sensor would not help them.
  `power_cycle_after(n)` restarts once it reaches `n`, so a sensor that keeps failing is
  restarted every `n` failures.
- The power pin is high for on and may be of any pin type; a failed restart tells which pin
  failed through `PowerError`. The sensor draws at most about 2.5 mA, which a GPIO can supply.

## The platform

A read blocks for about 23 ms: an 18 ms start pulse, then a 4 ms frame whose bits differ by
the length of a pulse, 26–28 µs for a 0 and 70 µs for a 1. Telling them apart takes two
things that `embedded-hal` 1.0 does not define, and the `Platform` trait asks for them:

- `ticks()` and `ticks_per_us()`: a free-running counter that counts up, wraps from
  `u32::MAX` to 0 and keeps counting with interrupts masked, such as the CPU cycle counter.
  One tick per microsecond is enough. If the counter stops, the read fails after a bounded
  number of polls instead of hanging.
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

[`examples/esp-idf`](https://github.com/marcelobns/dht11-trace/blob/main/examples/esp-idf) is a complete program for the ESP32.

## Timing

| Step | Value | Reason |
| --- | --- | --- |
| Start pulse | 18 ms low | Datasheet minimum |
| Acknowledgement check | 48 µs after release | Inside the 80 µs low answer, which starts 20–40 µs after release |
| Edge timeout | 100 µs | The longest phase of the protocol lasts 80 µs |
| 0 or 1 | High pulse longer than 48 µs is a 1 | 0 bits last 26–28 µs, 1 bits 70 µs |

The acknowledgement is checked at a fixed instant, timed by the counter inside the
uninterrupted section, instead of waiting for the line to rise after release: with a weak pull-up, the line may not reach the high level before the sensor
pulls it low, and the frame would come out shifted by one bit.

## Testing

`cargo test` runs the driver against a simulated line, with no hardware: valid frames, every
fault, a weak pull-up, bits away from the datasheet lengths, a 1 MHz counter, a counter that
wraps around during the frame, a counter that is misreported or stops, and a stuck sensor that
only recovers after losing power with DATA held low.

## Status

Checked on hardware with one setup: a classic ESP32 (revision 3.1, 160 MHz, ESP-IDF v5.5.5)
and one DHT11 module on GPIO4, through [`examples/esp-idf`](https://github.com/marcelobns/dht11-trace/blob/main/examples/esp-idf), with Wi-Fi off:
89 reads in 3 minutes, none failed. The same algorithm read the sensor with Wi-Fi traffic in
SenseBridge. The timing constants still need checking on other boards and sensors, and the
restart of a stuck sensor has only run against the simulated line so far; reports are
welcome.

## Origin

The driver started in the reference firmware of
[SenseBridge](https://github.com/msc-sed-ufrr/SenseBridge), where it reads a DHT11 on an
ESP32 next to Wi-Fi traffic.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/marcelobns/dht11-trace/blob/main/LICENSE-APACHE) or
[MIT license](https://github.com/marcelobns/dht11-trace/blob/main/LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above,
without any additional terms or conditions.
