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

/// Time without power in a power cycle.
const POWER_OFF_MS: u32 = 1000;

/// Time the DHT11 needs after power-up before it answers.
const POWER_UP_MS: u32 = 1000;

/// DHT11 on its single data line, powered from a fixed supply or from a pin.
///
/// The data pin must be open drain with a pull-up, and reading it must return the level of the
/// line while the pin is released. DHT11 modules usually carry the pull-up; a bare sensor
/// needs one of 4.7–10 kΩ to VCC.
///
/// The caller keeps the sensor's timing rules: wait 1 s after power-up before the first read
/// and at least 1 s between reads.
pub struct Dht11<P, D, T, W = FixedSupply> {
    pin: P,
    delay: D,
    platform: T,
    power: W,
    /// Failed reads in a row since the last good read or power cycle.
    failures: u32,
}

/// Power from a supply that [`Dht11`] cannot switch, such as the board's 3.3 V rail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FixedSupply;

impl<P, D, T> Dht11<P, D, T, FixedSupply>
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
            power: FixedSupply,
            failures: 0,
        })
    }

    /// Powers the sensor from `power`, high for on, so that [`Dht11::power_cycle`] can restart
    /// it. The 1 s power-up time starts here.
    ///
    /// A GPIO can supply the sensor, which draws at most about 2.5 mA.
    pub fn with_power<W>(self, mut power: W) -> Result<Dht11<P, D, T, W>, P::Error>
    where
        W: OutputPin<Error = P::Error>,
    {
        power.set_high()?;
        Ok(Dht11 {
            pin: self.pin,
            delay: self.delay,
            platform: self.platform,
            power,
            failures: self.failures,
        })
    }
}

impl<P, D, T, W> Dht11<P, D, T, W>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
{
    /// Reads one frame.
    ///
    /// Blocks for about 23 ms: the 18 ms start pulse, then about 4 ms inside
    /// [`Platform::uninterrupted`]. On failure, the [`Fault`] tells which stage of the protocol
    /// went wrong.
    pub fn read(&mut self) -> Result<Reading, Fault<P::Error>> {
        let result = self.read_once();
        self.failures = match result {
            Ok(_) => 0,
            Err(_) => self.failures.saturating_add(1),
        };
        result
    }

    /// Failed reads in a row since the last good read or power cycle.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    /// Gives back the data pin, the delay, the platform and the power pin.
    pub fn release(self) -> (P, D, T, W) {
        (self.pin, self.delay, self.platform, self.power)
    }

    fn read_once(&mut self) -> Result<Reading, Fault<P::Error>> {
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
            ..
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
}

impl<P, D, T, W> Dht11<P, D, T, W>
where
    P: InputPin + OutputPin,
    D: DelayNs,
    T: Platform,
    W: OutputPin<Error = P::Error>,
{
    /// Restarts the sensor: cuts its power for 1 s, restores it and waits the 1 s power-up
    /// time. Blocks for about 2 s.
    ///
    /// A stuck DHT11 only answers again after losing power. DATA is held low while the power is
    /// off; otherwise the pull-up could keep the sensor powered through it. The power is
    /// restored even if a pin fails, and the first error is returned.
    pub fn power_cycle(&mut self) -> Result<(), P::Error> {
        let off = self.pin.set_low().and(self.power.set_low());
        self.delay.delay_ms(POWER_OFF_MS);
        let on = self.power.set_high().and(self.pin.set_high());
        self.delay.delay_ms(POWER_UP_MS);
        self.failures = 0;
        off.and(on)
    }

    /// Restarts the sensor once `limit` reads in a row have failed, and tells whether it did.
    ///
    /// Call it between reads, outside any time-critical work: a restart blocks for about 2 s.
    /// The count starts over after a restart, so a sensor that keeps failing is restarted
    /// again every `limit` failures. A `limit` of 0 counts as 1.
    pub fn power_cycle_after(&mut self, limit: u32) -> Result<bool, P::Error> {
        if self.failures < limit.max(1) {
            return Ok(false);
        }
        self.power_cycle()?;
        Ok(true)
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
