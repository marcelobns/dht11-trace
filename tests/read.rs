mod sim;

use dht11_trace::{Dht11, Fault, Reading};
use sim::{answer, answer_cut, answer_with_bits, frame, Setup, Sim, SimError};

fn read(setup: Setup) -> (Result<Reading, Fault<SimError>>, Sim) {
    let sim = Sim::new(setup);
    let (pin, delay, platform) = sim.parts();
    let mut dht11 = Dht11::new(pin, delay, platform).unwrap();
    (dht11.read(), sim)
}

fn answering(answer: Vec<sim::Stretch>) -> Setup {
    Setup {
        answer,
        ..Setup::default()
    }
}

#[test]
fn reads_humidity_and_temperature() {
    let (result, _) = read(answering(answer(frame(50, 27))));
    let reading = result.unwrap();
    assert_eq!(reading.humidity(), 50);
    assert_eq!(reading.temperature(), 27);
    assert_eq!(reading.bytes(), [50, 0, 27, 0]);
}

#[test]
fn takes_bit_7_of_the_temperature_as_the_sign() {
    let (result, _) = read(answering(answer(frame(40, 0x85))));
    assert_eq!(result.unwrap().temperature(), -5);
}

#[test]
fn keeps_the_decimal_bytes() {
    let (result, _) = read(answering(answer([45, 0, 23, 4, 72])));
    assert_eq!(result.unwrap().bytes(), [45, 0, 23, 4]);
}

#[test]
fn reads_with_a_one_megahertz_counter() {
    let setup = Setup {
        ticks_per_us: 1,
        ..answering(answer(frame(61, 19)))
    };
    let (result, _) = read(setup);
    assert_eq!(result.unwrap().bytes(), [61, 0, 19, 0]);
}

#[test]
fn reads_while_the_counter_wraps_around() {
    let setup = Setup {
        // The counter wraps about 2 ms into the frame.
        first_tick: u32::MAX - 160 * 20_000,
        ..answering(answer(frame(50, 27)))
    };
    let (result, _) = read(setup);
    assert_eq!(result.unwrap().temperature(), 27);
}

#[test]
fn tolerates_bit_lengths_away_from_the_datasheet() {
    for (zero_us, one_us) in [(20, 80), (40, 56)] {
        let (result, _) = read(answering(answer_with_bits(
            frame(0xAA, 0x55),
            zero_us,
            one_us,
        )));
        assert_eq!(
            result.unwrap().bytes(),
            [0xAA, 0, 0x55, 0],
            "{zero_us} µs and {one_us} µs"
        );
    }
}

#[test]
fn reads_through_a_weak_pull_up() {
    // The line has not risen yet when the sensor pulls it low: no high level between the
    // release and the acknowledgement.
    let mut answer = answer(frame(50, 27));
    answer[0] = (false, 30);
    let (result, _) = read(answering(answer));
    assert_eq!(result.unwrap().bytes(), [50, 0, 27, 0]);
}

#[test]
fn reads_the_frame_without_interruption() {
    let (result, sim) = read(answering(answer(frame(50, 27))));
    assert!(result.is_ok());
    assert_eq!(sim.exposed_reads(), 0);
    // The 18 ms start pulse stays outside; the frame itself takes about 4 ms.
    assert!(sim.longest_uninterrupted_ns() < 6_000_000);
}

#[test]
fn reports_a_line_held_low() {
    let setup = Setup {
        stuck_low: true,
        ..answering(answer(frame(50, 27)))
    };
    assert_eq!(read(setup).0, Err(Fault::LineLow));
}

#[test]
fn reports_a_sensor_that_does_not_answer() {
    assert_eq!(read(Setup::default()).0, Err(Fault::NoAck));
}

#[test]
fn reports_an_acknowledgement_that_never_ends() {
    assert_eq!(
        read(answering(vec![(true, 30), (false, 1000)])).0,
        Err(Fault::AckBroken)
    );
}

#[test]
fn reports_an_acknowledgement_left_high() {
    assert_eq!(
        read(answering(vec![(true, 30), (false, 80)])).0,
        Err(Fault::AckBroken)
    );
}

#[test]
fn reports_how_many_bits_arrived() {
    for bits in [0, 1, 8, 39] {
        assert_eq!(
            read(answering(answer_cut(frame(50, 27), bits))).0,
            Err(Fault::Truncated { bits: bits as u8 })
        );
    }
}

#[test]
fn reports_a_checksum_mismatch_with_the_frame() {
    let bad = [50, 0, 27, 0, 78];
    assert_eq!(
        read(answering(answer(bad))).0,
        Err(Fault::Checksum { frame: bad })
    );
}

#[test]
fn reports_pin_errors() {
    let setup = Setup {
        broken_pin: true,
        ..Setup::default()
    };
    assert_eq!(read(setup).0, Err(Fault::Pin(SimError)));
}

#[test]
fn describes_each_fault() {
    let text = |fault: Fault<SimError>| fault.to_string();
    assert_eq!(
        text(Fault::Truncated { bits: 39 }),
        "frame cut after 39 of 40 bits"
    );
    assert_eq!(
        text(Fault::Checksum {
            frame: [50, 0, 27, 0, 78]
        }),
        "checksum mismatch in frame [32, 00, 1b, 00, 4e]"
    );
    assert_eq!(text(Fault::Pin(SimError)), "pin error: SimError");
    assert!(text(Fault::NoAck).starts_with("no answer to the start pulse"));
}

#[test]
fn hands_back_its_parts() {
    let sim = Sim::new(Setup::default());
    let (pin, delay, platform) = sim.parts();
    let dht11 = Dht11::new(pin, delay, platform).unwrap();
    let (_pin, _delay, _platform) = dht11.release();
}
