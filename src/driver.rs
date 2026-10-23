use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};

use crate::{Fault, Platform, Reading};

/// Start pulse: the datasheet asks for the line held low for at least 18 ms.
const START_PULSE_MS: u32 = 18;

/// When the line is sampled after release, inside the sensor's acknowledgement, which starts
/// 20–40 µs after release and stays low for 80 µs.
const ACK_SAMPLE_US: u32 = 48;

/// Longest wait for an edge. Every phase of the protocol lasts between 20 µs and 80 µs.
const EDGE_TIMEOUT_US: u32 = 100;

/// High pulses longer than this are 1 bits (70 µs); shorter ones are 0 bits (26–28 µs).
const ONE_THRESHOLD_US: u32 = 48;

/// DHT11 on its single data line.
///
/// The pin must be open drain with a pull-up, and reading it must return the level of the
/// line while the pin is released. DHT11 modules usually carry the pull-up; a bare sensor
/// needs one of 4.7–10 kΩ to VCC.
///
/// The caller keeps the sensor's timing rules: wait 1 s after power-up before the first read
/// and at least 1 s between reads.
pub struct Dht11<P, D, T> {
    pin: P,
    delay: D,
    platform: T,
}

impl<P, D, T> Dht11<P, D, T>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
{
    /// Takes the data pin and releases the line: the sensor takes a low level as a read
    /// request.
    pub fn new(mut pin: P, delay: D, platform: T) -> Result<Self, P::Error> {
        pin.set_high()?;
        Ok(Self {
            pin,
            delay,
            platform,
        })
    }

    /// Reads one frame.
    ///
    /// Blocks for about 23 ms: the 18 ms start pulse, then about 4 ms inside
    /// [`Platform::uninterrupted`]. On failure, the [`Fault`] tells which stage of the protocol
    /// went wrong.
    pub fn read(&mut self) -> Result<Reading, Fault<P::Error>> {
        if self.pin.is_low().map_err(Fault::Pin)? {
            return Err(Fault::LineLow);
        }
        self.pin.set_low().map_err(Fault::Pin)?;
        // The start pulse stays outside the uninterrupted section, which would otherwise block
        // every interrupt for 18 ms for no benefit.
        self.delay.delay_ms(START_PULSE_MS);

        let Self {
            pin,
            delay,
            platform,
        } = self;
        let platform = &*platform;
        let frame = platform.uninterrupted(|| read_frame(pin, delay, platform))?;

        let [data @ .., checksum] = frame;
        if data.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) == checksum {
            Ok(Reading::new(data))
        } else {
            Err(Fault::Checksum { frame })
        }
    }

    /// Gives back the pin, the delay and the platform.
    pub fn release(self) -> (P, D, T) {
        (self.pin, self.delay, self.platform)
    }
}

/// Releases the line and reads the 40-bit frame: humidity, its decimal part, temperature, its
/// decimal part and the checksum.
fn read_frame<P, D, T>(pin: &mut P, delay: &mut D, platform: &T) -> Result<[u8; 5], Fault<P::Error>>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
{
    pin.set_high().map_err(Fault::Pin)?;
    // The line is sampled in the middle of the acknowledgement instead of waiting for it to
    // rise after release: with a weak pull-up it may not reach the high level before the
    // sensor pulls it low, and the frame would come out shifted by one bit.
    delay.delay_us(ACK_SAMPLE_US);
    if pin.is_high().map_err(Fault::Pin)? {
        return Err(Fault::NoAck);
    }
    let mut line = Line::new(pin, platform);
    line.wait_for(Level::High, Fault::AckBroken)?;
    line.wait_for(Level::Low, Fault::AckBroken)?;

    // Each bit: 50 µs low, then a high pulse whose length gives the value.
    let mut frame = [0u8; 5];
    for bit in 0..40u8 {
        let rise = line.wait_for(Level::High, Fault::Truncated { bits: bit })?;
        let fall = line.wait_for(Level::Low, Fault::Truncated { bits: bit })?;
        let byte = &mut frame[usize::from(bit / 8)];
        *byte = *byte << 1 | u8::from(line.micros(rise, fall) > ONE_THRESHOLD_US);
    }
    Ok(frame)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Low,
    High,
}

/// The data line, timed by the platform's counter.
struct Line<'a, P, T> {
    pin: &'a mut P,
    platform: &'a T,
    ticks_per_us: u32,
}

impl<'a, P: InputPin, T: Platform> Line<'a, P, T> {
    fn new(pin: &'a mut P, platform: &'a T) -> Self {
        Self {
            pin,
            platform,
            ticks_per_us: platform.ticks_per_us().max(1),
        }
    }

    /// Waits for the line to reach `level` and returns the counter at that moment, or
    /// `timeout` after [`EDGE_TIMEOUT_US`].
    fn wait_for(&mut self, level: Level, timeout: Fault<P::Error>) -> Result<u32, Fault<P::Error>> {
        let start = self.platform.ticks();
        let limit = EDGE_TIMEOUT_US * self.ticks_per_us;
        loop {
            let now = self.platform.ticks();
            let high = self.pin.is_high().map_err(Fault::Pin)?;
            if high == (level == Level::High) {
                return Ok(now);
            }
            if now.wrapping_sub(start) > limit {
                return Err(timeout);
            }
        }
    }

    /// Microseconds between two counter values, even if the counter wrapped in between.
    fn micros(&self, from: u32, to: u32) -> u32 {
        to.wrapping_sub(from) / self.ticks_per_us
    }
}
