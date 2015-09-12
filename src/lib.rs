#![recursion_limit = "1000"]
#![cfg_attr(test, feature(test))]

#![allow(dead_code)] // TODO: remove

#[macro_use]
extern crate nom;

extern crate byteorder;

mod loadavg;
mod parsers;
pub mod pid;

pub use loadavg::{LoadAvg, loadavg};
