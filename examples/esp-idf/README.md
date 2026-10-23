# dht11-trace on the ESP32 (ESP-IDF)

Reads a DHT11 every 2 s and logs the reading or the stage where it failed, with a count of
failed reads every minute. It runs on the classic ESP32 with ESP-IDF v5.5.5 and `std`, and
implements `Platform` with the CPU cycle counter and an `IsrCriticalSection`.

## Wiring

| DHT11 | ESP32 |
| --- | --- |
| VCC | 3V3 |
| DATA | GPIO4 (D4 on the DevKit) |
| GND | GND |

DHT11 modules carry the pull-up on DATA; a bare sensor needs 4.7–10 kΩ between DATA and VCC.
The pin is configured as open drain with the internal pull-up as well.

## Build and flash

The project builds with the Xtensa toolchain from [espup](https://github.com/esp-rs/espup),
outside the crate:

```sh
cargo install espup espflash ldproxy --locked
espup install
. ~/export-esp.sh
cd examples/esp-idf
cargo run --release
```

The first build downloads ESP-IDF v5.5.5 to `~/.espressif` and takes a few minutes.
`cargo run` flashes the board and opens the serial monitor.

## Output

```text
I (362) dht11_trace_esp_idf: DHT11 on GPIO4, one read every 2 s
I (1402) dht11_trace_esp_idf: 27 °C, 43 % (bytes [43, 0, 27, 6])
I (3432) dht11_trace_esp_idf: 27 °C, 43 % (bytes [43, 0, 27, 6])
...
I (60272) dht11_trace_esp_idf: 0 failed reads out of 30
```

A failed read logs a warning with the stage, such as `DHT11: no answer to the start pulse
(sensor unpowered, data line disconnected or sensor stuck)`.

Unplugging the sensor while it runs shows `NoAck`; shorting DATA to GND shows `LineLow`.
