#![doc = include_str!("../README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod driver;
mod fault;
mod platform;
mod reading;

pub use driver::{Dht11, FixedSupply};
pub use fault::{Fault, PowerError};
pub use platform::Platform;
pub use reading::Reading;
