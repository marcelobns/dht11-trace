mod sim;

use dht11_trace::{Dht11, Fault};
use sim::{answer, frame, Setup, Sim, SimDelay, SimError, SimPin, SimPlatform, SimPower};

type PoweredDht11 = Dht11<SimPin, SimDelay, SimPlatform, SimPower>;

fn powered(setup: Setup) -> (PoweredDht11, Sim) {
    let sim = Sim::new(setup);
    let (pin, delay, platform) = sim.parts();
    let dht11 = Dht11::new(pin, delay, platform)
        .unwrap()
        .with_power(sim.power())
        .unwrap();
    (dht11, sim)
}

fn stuck_sensor() -> (PoweredDht11, Sim) {
    powered(Setup {
        stuck: true,
        answer: answer(frame(50, 27)),
        ..Setup::default()
    })
}

#[test]
fn power_cycle_restarts_a_stuck_sensor() {
    let (mut dht11, sim) = stuck_sensor();
    assert_eq!(dht11.read(), Err(Fault::NoAck));

    let before = sim.now_ns();
    dht11.power_cycle().unwrap();
    // 1 s without power and 1 s for the sensor to power up. The simulated sensor only
    // recovers if DATA is held low as well, and only answers 1 s after power-up.
    assert!(sim.now_ns() - before >= 2_000_000_000);
    assert_eq!(dht11.read().unwrap().temperature(), 27);
}

#[test]
fn counts_failed_reads_in_a_row() {
    let (mut dht11, _sim) = stuck_sensor();
    for expected in 1..=3 {
        let _ = dht11.read();
        assert_eq!(dht11.failures(), expected);
    }
    dht11.power_cycle().unwrap();
    assert_eq!(dht11.failures(), 0);
    dht11.read().unwrap();
    assert_eq!(dht11.failures(), 0);
}

#[test]
fn power_cycles_after_the_given_number_of_failures() {
    let (mut dht11, _sim) = stuck_sensor();
    for _ in 0..2 {
        let _ = dht11.read();
        assert!(!dht11.power_cycle_after(3).unwrap());
    }
    let _ = dht11.read();
    assert!(dht11.power_cycle_after(3).unwrap());
    // The count started over: no second restart before more failures.
    assert!(!dht11.power_cycle_after(3).unwrap());
    assert!(dht11.read().is_ok());
}

#[test]
fn restores_the_power_even_when_a_pin_fails() {
    let (mut dht11, sim) = powered(Setup {
        broken_pin: true,
        ..Setup::default()
    });
    assert_eq!(dht11.power_cycle(), Err(SimError));
    assert!(sim.powered());
}
