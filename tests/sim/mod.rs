//! A simulated DHT11 line: a pin, a delay and a platform that share one clock.
//!
//! Time advances only when the driver waits or reads the pin, so every test is deterministic.

use std::cell::Cell;
use std::rc::Rc;

use dht11_trace::Platform;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{self, ErrorKind, ErrorType, InputPin, OutputPin};

/// What reading the pin costs, in nanoseconds: the polling loops advance the clock.
const READ_NS: u64 = 300;

/// Shortest start pulse the simulated sensor answers.
const START_PULSE_NS: u64 = 18_000_000;

/// A stretch of the sensor's answer: whether the line is high, and for how many microseconds.
pub type Stretch = (bool, u64);

/// The datasheet's answer to a start pulse: 30 µs before the acknowledgement, 80 µs low,
/// 80 µs high, the 40 bits and a final 50 µs low.
pub fn answer(frame: [u8; 5]) -> Vec<Stretch> {
    answer_with_bits(frame, 27, 70)
}

/// [`answer`] with other lengths for the high pulse of 0 and 1 bits.
pub fn answer_with_bits(frame: [u8; 5], zero_us: u64, one_us: u64) -> Vec<Stretch> {
    let mut answer = vec![(true, 30), (false, 80), (true, 80)];
    for byte in frame {
        for shift in (0..8).rev() {
            answer.push((false, 50));
            answer.push((
                true,
                if byte >> shift & 1 == 1 {
                    one_us
                } else {
                    zero_us
                },
            ));
        }
    }
    answer.push((false, 50));
    answer
}

/// [`answer`] cut after `bits` complete bits: the sensor pulls the line low to end the last
/// one and then lets it go.
pub fn answer_cut(frame: [u8; 5], bits: usize) -> Vec<Stretch> {
    let mut answer = answer(frame);
    answer.truncate(3 + 2 * bits + 1);
    answer
}

/// A frame with a valid checksum and zero decimal bytes.
pub fn frame(humidity: u8, temperature: u8) -> [u8; 5] {
    [
        humidity,
        0,
        temperature,
        0,
        humidity.wrapping_add(temperature),
    ]
}

/// How the simulated line behaves.
pub struct Setup {
    /// The sensor's answer to a start pulse; after it, the pull-up holds the line high.
    pub answer: Vec<Stretch>,
    pub ticks_per_us: u32,
    /// Counter value at time zero, to test wrap-around.
    pub first_tick: u32,
    /// Something holds the line low.
    pub stuck_low: bool,
    /// `set_low` fails.
    pub broken_pin: bool,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            answer: Vec::new(),
            ticks_per_us: 160,
            first_tick: 0,
            stuck_low: false,
            broken_pin: false,
        }
    }
}

struct State {
    setup: Setup,
    now_ns: Cell<u64>,
    /// Since when the host has been holding the line low.
    host_low_since: Cell<Option<u64>>,
    /// When the sensor started its answer.
    answer_from: Cell<Option<u64>>,
    uninterrupted: Cell<bool>,
    /// Pin reads after the start pulse that happened outside `uninterrupted`.
    exposed_reads: Cell<u32>,
    /// Longest stretch spent inside `uninterrupted`, in nanoseconds.
    longest_uninterrupted_ns: Cell<u64>,
}

impl State {
    fn line_high(&self) -> bool {
        if self.setup.stuck_low || self.host_low_since.get().is_some() {
            return false;
        }
        let Some(from) = self.answer_from.get() else {
            return true;
        };
        let mut elapsed = self.now_ns.get() - from;
        for &(high, us) in &self.setup.answer {
            if elapsed < us * 1000 {
                return high;
            }
            elapsed -= us * 1000;
        }
        true
    }
}

#[derive(Clone)]
pub struct Sim(Rc<State>);

impl Sim {
    pub fn new(setup: Setup) -> Self {
        Self(Rc::new(State {
            setup,
            now_ns: Cell::new(0),
            host_low_since: Cell::new(None),
            answer_from: Cell::new(None),
            uninterrupted: Cell::new(false),
            exposed_reads: Cell::new(0),
            longest_uninterrupted_ns: Cell::new(0),
        }))
    }

    pub fn parts(&self) -> (SimPin, SimDelay, SimPlatform) {
        (
            SimPin(self.0.clone()),
            SimDelay(self.0.clone()),
            SimPlatform(self.0.clone()),
        )
    }

    pub fn exposed_reads(&self) -> u32 {
        self.0.exposed_reads.get()
    }

    pub fn longest_uninterrupted_ns(&self) -> u64 {
        self.0.longest_uninterrupted_ns.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimError;

impl digital::Error for SimError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

pub struct SimPin(Rc<State>);

impl ErrorType for SimPin {
    type Error = SimError;
}

impl OutputPin for SimPin {
    fn set_low(&mut self) -> Result<(), SimError> {
        if self.0.setup.broken_pin {
            return Err(SimError);
        }
        if self.0.host_low_since.get().is_none() {
            self.0.host_low_since.set(Some(self.0.now_ns.get()));
        }
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), SimError> {
        let now = self.0.now_ns.get();
        if let Some(since) = self.0.host_low_since.take() {
            if now - since >= START_PULSE_NS {
                self.0.answer_from.set(Some(now));
            }
        }
        Ok(())
    }
}

impl InputPin for SimPin {
    fn is_high(&mut self) -> Result<bool, SimError> {
        let state = &self.0;
        state.now_ns.set(state.now_ns.get() + READ_NS);
        if state.answer_from.get().is_some() && !state.uninterrupted.get() {
            state.exposed_reads.set(state.exposed_reads.get() + 1);
        }
        Ok(state.line_high())
    }

    fn is_low(&mut self) -> Result<bool, SimError> {
        self.is_high().map(|high| !high)
    }
}

pub struct SimDelay(Rc<State>);

impl DelayNs for SimDelay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.now_ns.set(self.0.now_ns.get() + u64::from(ns));
    }
}

pub struct SimPlatform(Rc<State>);

impl Platform for SimPlatform {
    fn ticks(&self) -> u32 {
        let state = &self.0;
        let elapsed = state.now_ns.get() * u64::from(state.setup.ticks_per_us) / 1000;
        state.setup.first_tick.wrapping_add(elapsed as u32)
    }

    fn ticks_per_us(&self) -> u32 {
        self.0.setup.ticks_per_us
    }

    fn uninterrupted<R>(&self, read: impl FnOnce() -> R) -> R {
        let state = &self.0;
        let entered = state.now_ns.get();
        state.uninterrupted.set(true);
        let result = read();
        state.uninterrupted.set(false);
        let spent = state.now_ns.get() - entered;
        state
            .longest_uninterrupted_ns
            .set(state.longest_uninterrupted_ns.get().max(spent));
        result
    }
}
