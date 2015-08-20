//! Parsers and data structures for `/proc/[pid]/statm`.

use std::fs::File;
use std::io::{Error, ErrorKind, Result};
use std::os::unix::raw::pid_t;

use nom::{IResult, digit, line_ending, space};

use parsers::{parse_usize, read_to_end};

/// Provides information about memory usage, as measured in pages.
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

/// Parses the statm file format.
named!(parse_statm<Statm>,
    chain!(
        size: parse_usize     ~ space ~
        resident: parse_usize ~ space ~
        share: parse_usize    ~ space ~
        text: parse_usize     ~ space ~
        digit                 ~ space ~         // lib - unused since linux 2.6
        data: parse_usize     ~ space ~
        digit                 ~ line_ending,    // dt - unused since linux 2.6
        || { Statm { size: size,
                     resident: resident,
                     share: share,
                     text: text,
                     data: data } }));

/// Parses the provided statm file.
fn statm_file(file: &mut File) -> Result<Statm> {
    let mut buf = [0; 256]; // A typical statm file is about 25 bytes
    match parse_statm(try!(read_to_end(file, &mut buf))) {
        IResult::Done(_, statm) => Ok(statm),
        _ => Err(Error::new(ErrorKind::InvalidData, "unable to parse statm file")),
    }
}

/// Returns memory status information for the process with the provided pid.
pub fn statm(pid: pid_t) -> Result<Statm> {
    statm_file(&mut try!(File::open(&format!("/proc/{}/statm", pid))))
}

/// Returns memory status information for the current process.
pub fn statm_self() -> Result<Statm> {
    statm_file(&mut try!(File::open("/proc/self/statm")))
}

#[cfg(test)]
mod tests {

    extern crate test;

    use std::fs::File;
    use std::str;

    use nom::IResult;

    use parsers::read_to_end;
    use super::{Statm, parse_statm, statm, statm_self};

    /// Tests that the statm function returns non-zero memory values for the init process.
    #[test]
    fn test_statm() {
        statm_self().unwrap();
        let Statm { size, resident, share, text, data } = statm(1).unwrap();
        assert!(size != 0);
        assert!(resident != 0);
        assert!(share != 0);
        assert!(text != 0);
        assert!(data != 0);
    }

    #[test]
    fn test_parse_statm() {
        let mut buf = [0; 256];
        let status = read_to_end(&mut File::open("/proc/1/statm").unwrap(), &mut buf).unwrap();

        match parse_statm(status) {
            IResult::Done(remaining, _) => {
                if !remaining.is_empty() {
                    panic!(format!("Unable to parse whole status file, remaining:\n{}",
                                   str::from_utf8(remaining).unwrap()));
                }
            }
            _ => panic!("unable to unwrap IResult"),
        }
    }

    #[bench]
    fn bench_statm(b: &mut test::Bencher) {
        b.iter(|| test::black_box(statm(1)));
    }

    #[bench]
    fn bench_parse_statm(b: &mut test::Bencher) {
        let mut buf = [0; 256];
        let statm = read_to_end(&mut File::open("/proc/1/statm").unwrap(), &mut buf).unwrap();
        b.iter(|| test::black_box(parse_statm(statm)));
    }
}
