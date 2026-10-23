use core::fmt;

/// Stage of the protocol where a read failed.
///
/// New stages may be added in minor versions, so a `match` needs a wildcard arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Fault<E> {
    /// The line was already low before the start pulse: DATA shorted to ground or the sensor
    /// stuck.
    LineLow,
    /// The sensor did not answer the start pulse: no power, DATA disconnected or the sensor
    /// stuck.
    NoAck,
    /// The answer started but did not follow the 80 µs low and 80 µs high of the datasheet.
    AckBroken,
    /// The frame stopped after `bits` of its 40 bits.
    Truncated {
        /// Bits received before the line stopped changing.
        bits: u8,
    },
    /// The frame arrived complete, but the checksum does not match.
    Checksum {
        /// The five bytes received: four data bytes and the checksum.
        frame: [u8; 5],
    },
    /// All 40 bits read as 0. The checksum matches, but a working DHT11 never sends this
    /// frame: the pulses were measured too short, usually because
    /// [`Platform::ticks_per_us`](crate::Platform::ticks_per_us) overstates the counter.
    ZeroFrame,
    /// The pin returned an error.
    Pin(E),
}

impl<E: fmt::Debug> fmt::Display for Fault<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineLow => f.write_str(
                "data line held low before the start pulse (shorted to ground or sensor stuck)",
            ),
            Self::NoAck => f.write_str(
                "no answer to the start pulse \
                 (sensor unpowered, data line disconnected or sensor stuck)",
            ),
            Self::AckBroken => f.write_str("incomplete answer to the start pulse"),
            Self::Truncated { bits } => write!(f, "frame cut after {bits} of 40 bits"),
            Self::Checksum { frame } => write!(f, "checksum mismatch in frame {frame:02x?}"),
            Self::ZeroFrame => {
                f.write_str("all 40 bits read as 0 (check the counter's ticks per µs)")
            }
            Self::Pin(error) => write!(f, "pin error: {error:?}"),
        }
    }
}

impl<E: fmt::Debug> core::error::Error for Fault<E> {}

/// A pin error while restarting the sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerError<D, W> {
    /// The data pin failed.
    Data(D),
    /// The power pin failed.
    Power(W),
}

impl<D: fmt::Debug, W: fmt::Debug> fmt::Display for PowerError<D, W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Data(error) => write!(f, "data pin error: {error:?}"),
            Self::Power(error) => write!(f, "power pin error: {error:?}"),
        }
    }
}

impl<D: fmt::Debug, W: fmt::Debug> core::error::Error for PowerError<D, W> {}
