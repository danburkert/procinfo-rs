//! Parsers and data structures for `/proc/loadavg`.

use std::fs::File;
use std::io::{Error, ErrorKind, Result};

use nom::{IResult, space};

use parsers::{parse_u32, parse_f32, read_to_end};

/// Provides information about the system load average figures
#[derive(Debug, Default, PartialEq)]
pub struct LoadAvg {
    /// Load average of the last minute
    pub load_avg_1_minute: f32,
    /// Load average of the last 5 minutes
    pub load_avg_5_minutes: f32,
    /// Load average of the last 10 minutes
    pub load_avg_10_minutes: f32,

    /// the number of currently runnable kernel scheduling entities (processes, threads)
    pub number_of_scheduled_entities: u32,
    /// the number of kernel scheduling entities that currently exist on the system
    pub number_of_total_entities: u32,
    /// the PID of the process that was most recently created on the system
    pub last_created_pid: u32
}

/// Parses the loadavg file format.
named!(parse_loadavg<LoadAvg>,
    dbg!(chain!(
        load_avg_1_minute: parse_f32     ~ space ~
        load_avg_5_minutes: parse_f32     ~ space ~
        load_avg_10_minutes: parse_f32     ~ space ~
        number_of_scheduled_entities: parse_u32 ~ tag!("/") ~
        number_of_total_entities: parse_u32 ~ space ~
        last_created_pid: parse_u32
        ,
        || { LoadAvg { load_avg_1_minute: load_avg_1_minute,
                     load_avg_5_minutes: load_avg_5_minutes,
                     load_avg_10_minutes: load_avg_10_minutes,
                     number_of_scheduled_entities: number_of_scheduled_entities,
                     number_of_total_entities: number_of_total_entities,
                     last_created_pid: last_created_pid,
                     } }
)));

/// Parses the provided loadavg file.
fn loadavg_file(file: &mut File) -> Result<LoadAvg> {
    let mut buf = [0; 256];
    match parse_loadavg(try!(read_to_end(file, &mut buf))) {
        IResult::Done(_, load_avg) => Ok(load_avg),
        IResult::Error(err) => Err(Error::new(ErrorKind::InvalidData, format!("unable to parse loadavg file {:?}", err))),
        _ => Err(Error::new(ErrorKind::InvalidData, "unable to parse loadavg file")),
    }
}

/// Returns system load averages
pub fn loadavg() -> Result<LoadAvg> {
    loadavg_file(&mut try!(File::open("/proc/loadavg")))
}

#[cfg(test)]
mod tests {

    use std::fs::File;

    use super::loadavg_file;

    /// Tests for the loadavg function with the provided test data
    #[test]
    fn test_loadavg() {
        let result = loadavg_file(&mut File::open("testdata/loadavg").unwrap()).unwrap();
        assert_eq!(0.46, result.load_avg_1_minute);
        assert_eq!(0.33, result.load_avg_5_minutes);
        assert_eq!(0.28, result.load_avg_10_minutes);
        assert_eq!(34, result.number_of_scheduled_entities);
        assert_eq!(625, result.number_of_total_entities);
        assert_eq!(8435, result.last_created_pid);
    }

}
