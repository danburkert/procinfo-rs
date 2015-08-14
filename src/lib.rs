#![cfg_attr(test, feature(test))]

#[macro_use]
extern crate nom;

use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result};
use std::os::unix::raw::pid_t;
use std::str::{self, FromStr};

use nom::{
    digit,
    space,
    IResult,
};

/// Provides information about memory usage, measured in pages.
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Statm {
    /// Total virtual memory size.
    pub size: usize,
    /// Resident non-swapped memory.
    pub resident: usize,
    /// Shared memory.
    pub share: usize,
    /// Resident executable memory.
    pub text: usize,
    /// Resident data and stack memory.
    pub data: usize,
}

named!(parse_usize<usize>,
       map_res!(map_res!(digit, str::from_utf8), FromStr::from_str));

/// Parse the statm file format.
named!(parse_statm<Statm>,
    chain!(
        size: parse_usize       ~ space ~
        resident: parse_usize   ~ space ~
        share: parse_usize      ~ space ~
        text: parse_usize       ~ space ~
        digit                   ~ space ~   // lib - unused since linux 2.6
        data: parse_usize       ~ space ~
        digit,                              // dt - unused since linux 2.6
        || { Statm { size: size,
                     resident: resident,
                     share: share,
                     text: text,
                     data: data } }));

/// Returns memory status information for the process with the provided pid.
pub fn statm(pid: pid_t) -> Result<Statm> {
    let mut file = try!(File::open(&format!("/proc/{}/statm", pid)));
    let mut line = String::with_capacity(32);
    try!(file.read_to_string(&mut line));
    match parse_statm(line.as_bytes()) {
        IResult::Done(_, statm) => Ok(statm),
        _                       => Err(Error::new(ErrorKind::InvalidData,
                                                  "unable to parse statm file")),
    }
}

/// Returns memory status information for the current process.
pub fn statm_self() -> Result<Statm> {
    let mut file = try!(File::open("/proc/self/statm"));
    let mut line = String::with_capacity(32);
    try!(file.read_to_string(&mut line));
    match parse_statm(line.as_bytes()) {
        IResult::Done(_, statm) => Ok(statm),
        _                       => Err(Error::new(ErrorKind::InvalidData,
                                                  "unable to parse statm file")),
    }
}

#[cfg(test)]
mod tests {

    extern crate test;

    use super::*;

    /// Tests that the statm function returns non-zero memory values for the init process.
    #[test]
    fn test_statm() {
        let Statm { size, resident, share, text, data } = statm(1).unwrap();
        assert!(size != 0);
        assert!(resident != 0);
        assert!(share != 0);
        assert!(text != 0);
        assert!(data != 0);
    }

    #[bench]
    fn bench_statm(b: &mut test::Bencher) {
        b.iter(|| test::black_box(statm(1)));
    }

    #[bench]
    fn bench_statm_self(b: &mut test::Bencher) {
        b.iter(|| test::black_box(statm_self()));
    }
}
