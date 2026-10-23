# Changelog

Notable changes to `dht11-trace`. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

First version, to be published as 0.1.0.

### Added

- `Dht11` driver over `embedded-hal` 1.0 pins and delays, with a `Platform` trait for the pulse counter and the uninterrupted frame read.
- `Fault`, naming the stage where a read failed: line held low, no acknowledgement, broken acknowledgement, frame cut after N bits, checksum mismatch with the received frame, or pin error.
- `Reading`, with the integral humidity and temperature and the four data bytes as received.
- Host tests against a simulated line.
