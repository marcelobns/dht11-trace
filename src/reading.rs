/// A frame with a valid checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    bytes: [u8; 4],
}

impl Reading {
    pub(crate) fn new(bytes: [u8; 4]) -> Self {
        Self { bytes }
    }

    /// Relative humidity in percent, integral part.
    #[must_use]
    pub fn humidity(&self) -> u8 {
        self.bytes[0]
    }

    /// Temperature in degrees Celsius, integral part.
    ///
    /// Bit 7 of the temperature byte is taken as the sign, as `dht-sensor` 0.3 does. The
    /// DHT11 is specified for 0–50 °C, so the bit only matters for parts that report below
    /// zero.
    #[must_use]
    pub fn temperature(&self) -> i8 {
        let magnitude = (self.bytes[2] & 0x7F) as i8;
        if self.bytes[2] & 0x80 == 0 {
            magnitude
        } else {
            -magnitude
        }
    }

    /// The four data bytes as received: humidity, its decimal part, temperature and its
    /// decimal part. Some parts send zero in the decimal bytes; others send tenths of a degree
    /// in the temperature's, such as `[43, 0, 27, 6]` for 27.6 °C.
    #[must_use]
    pub fn bytes(&self) -> [u8; 4] {
        self.bytes
    }
}
