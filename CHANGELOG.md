# Changelog

Notable changes to `dht11-trace`. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [0.1.0] - 2026-10-22

First version.

### Added

- `Dht11` driver over `embedded-hal` 1.0 pins and delays, with a `Platform` trait for the pulse counter and the uninterrupted frame read.
- `Fault`, naming the stage where a read failed: line held low, no acknowledgement, broken acknowledgement, frame cut after N bits, checksum mismatch with the received frame, an all-zero frame from pulses measured too short, or pin error. It is `#[non_exhaustive]`.
- `Reading`, with the integral humidity and temperature and the four data bytes as received.
- Restart of a stuck sensor powered from a pin: `Dht11::with_power`, `power_cycle` and `power_cycle_after`, which hold DATA low while the power is off and report pin errors as `PowerError`; `failures` counts failed reads in a row, pin errors aside.
- Host tests against a simulated line.

[0.1.0]: https://github.com/marcelobns/dht11-trace/releases/tag/v0.1.0
