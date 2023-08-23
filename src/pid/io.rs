// Copyright 2018 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

//! Concerning the I/O information of a process, from
//! `/proc/[pid]/io`.

use std::fs::File;
use std::io::Result;

use libc::pid_t;
use nom::{
    IResult,
    Err,
    ErrorKind,
    line_ending,
    space,
};

use parsers::{
    map_result,
    parse_usize,
    read_to_end,
};

/// The I/O information of a process
#[derive(Debug, Default, PartialEq, Eq, Hash)]
pub struct Io {
    pub rchar: usize,
    pub wchar: usize,
    pub syscr: usize,
    pub syscw: usize,
    pub read_bytes: usize,
    pub write_bytes: usize,
    pub cancelled_write_bytes: usize,
}

named!(opt_space<Option<&[u8]>>, opt!(space));
named!(parse_rchar<usize>, chain!(tag!("rchar:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_wchar<usize>, chain!(tag!("wchar:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_syscr<usize>, chain!(tag!("syscr:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_syscw<usize>, chain!(tag!("syscw:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_read_bytes<usize>, chain!(tag!("read_bytes:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_write_bytes<usize>, chain!(tag!("write_bytes:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));
named!(parse_cancelled_write_bytes<usize>, chain!(tag!("cancelled_write_bytes:") ~ opt_space ~ s: parse_usize ~ line_ending, || { s }));

fn parse_io(mut input: &[u8]) -> IResult<&[u8], Io> {
    let mut io: Io = Default::default();
    loop {
        let original_len = input.len();
        let (rest, ()) = try_parse!(input,
            alt!( parse_rchar                 => { |value| io.rchar = value }
                | parse_wchar                 => { |value| io.wchar = value }
                | parse_syscr                 => { |value| io.syscr = value }
                | parse_syscw                 => { |value| io.syscw = value }
                | parse_read_bytes            => { |value| io.read_bytes = value }
                | parse_write_bytes           => { |value| io.write_bytes = value }
                | parse_cancelled_write_bytes => { |value| io.cancelled_write_bytes = value }
            )
        );
        let final_len = rest.len();
        if final_len == 0 {
            break IResult::Done(&[], io);
        } else if original_len == final_len {
            break IResult::Error(Err::Position(ErrorKind::Tag, rest));
        }
        input = rest;
    }
}

/// Parses the provided stat file.
fn io_file(file: &mut File) -> Result<Io> {
    let mut buf = [0; 256]; // A typical io file is about 100 bytes
    map_result(parse_io(read_to_end(file, &mut buf)?))
}

/// Returns I/O information for the process with the provided pid.
pub fn io(pid: pid_t) -> Result<Io> {
    io_file(&mut File::open(&format!("/proc/{}/io", pid))?)
}

/// Returns I/O information for the current process.
pub fn io_self() -> Result<Io> {
    io_file(&mut File::open("/proc/self/io")?)
}

/// Returns I/O information from the thread with the provided parent process ID and thread ID.
pub fn io_task(process_id: pid_t, thread_id: pid_t) -> Result<Io> {
    io_file(&mut File::open(&format!("/proc/{}/task/{}/io", process_id, thread_id))?)
}

#[cfg(test)]
pub mod tests {
    use parsers::tests::unwrap;
    use libc::getpid;
    use super::{Io, io, io_self, parse_io};

    #[test]
    fn test_io() {
        io_self().unwrap();
        io(unsafe { getpid() }).unwrap();
    }

    #[test]
    fn test_parse_io() {
        let text = b"rchar: 4685194216
wchar: 2920419824
syscr: 1687286
syscw: 708998
read_bytes: 2938340352
write_bytes: 2464854016
cancelled_write_bytes: 592056320
";
        let io: Io = unwrap(parse_io(text));
        assert_eq!(4685194216, io.rchar);
        assert_eq!(2920419824, io.wchar);
        assert_eq!(1687286,    io.syscr);
        assert_eq!(708998,     io.syscw);
        assert_eq!(2938340352, io.read_bytes);
        assert_eq!(2464854016, io.write_bytes);
        assert_eq!(592056320,  io.cancelled_write_bytes);
    }
}
