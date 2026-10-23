use core::fmt;

/// Stage of the protocol where a read failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            Self::Pin(error) => write!(f, "pin error: {error:?}"),
        }
    }
}

impl<E: fmt::Debug> core::error::Error for Fault<E> {}
