#![recursion_limit = "1000"]
#![cfg_attr(test, feature(test))]

#![allow(dead_code)] // TODO: remove

#[macro_use]
extern crate nom;

extern crate byteorder;
extern crate libc;

#[macro_use]
mod parsers;

mod loadavg;
pub mod pid;

pub use loadavg::{LoadAvg, loadavg};
