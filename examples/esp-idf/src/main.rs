//! Reads a DHT11 on GPIO4 every 2 s and logs the reading or the stage where it failed, with a
//! running count of failures.

use std::thread;
use std::time::Duration;

use dht11_trace::{Dht11, Platform};
use esp_idf_svc::hal::delay::Ets;
use esp_idf_svc::hal::gpio::{PinDriver, Pull};
use esp_idf_svc::hal::interrupt::IsrCriticalSection;
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::sys::{self, EspError};
use log::{info, warn};

/// The DHT11 needs at least 1 s between reads.
const INTERVAL: Duration = Duration::from_secs(2);

/// The sensor needs 1 s after power-up before the first read.
const POWER_UP: Duration = Duration::from_secs(1);

/// Reads between two summaries: one minute.
const SUMMARY_EVERY: u32 = 30;

/// The CPU cycle counter, which keeps counting with interrupts masked, and an ISR critical
/// section, which keeps the Wi-Fi task and interrupts off this core while the frame is read.
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

fn main() -> Result<(), EspError> {
    sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let pin = PinDriver::input_output_od(peripherals.pins.gpio4, Pull::Up)?;
    let platform = Esp32 {
        section: IsrCriticalSection::new(),
    };
    let mut dht11 = Dht11::new(pin, Ets, platform).expect("release the DHT11 line");
    info!("DHT11 on GPIO4, one read every {} s", INTERVAL.as_secs());
    thread::sleep(POWER_UP);

    let (mut reads, mut failures) = (0u32, 0u32);
    loop {
        reads += 1;
        match dht11.read() {
            Ok(reading) => info!(
                "{} °C, {} % (bytes {:?})",
                reading.temperature(),
                reading.humidity(),
                reading.bytes()
            ),
            Err(fault) => {
                failures += 1;
                warn!("DHT11: {fault}");
            }
        }
        if reads % SUMMARY_EVERY == 0 {
            info!("{failures} failed reads out of {reads}");
        }
        thread::sleep(INTERVAL);
    }
}
