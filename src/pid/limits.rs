//! Process limits informations from `/proc/[pid]/limits`.

use std::fs::File;
use std::io::Result;
use std::str::{self};
use std::time::Duration;

use libc::pid_t;
use nom::{
    IResult,
    is_space
};

use parsers::{
    map_result,
    parse_u64,
    parse_usize,
    read_to_end
};

enum Unit{
    Seconds,
    Microseconds
}

// consumes limit name and spaces before first value
named!(before_soft_value<&[u8], ()>,
    do_parse!(
        take_until!("  ") >>
        take_while!(is_space) >>
        ( () )
    )
);

// consumes between soft and hard value
named!(between_soft_hard_value, take_while!(is_space));
// consumes until we get a line break
named!(end_of_line, take_until_and_consume!(&b"\n"[..]));

named!(parse_usize_value<&[u8], Option<usize>>,
    alt!(
        tag!("unlimited") => { |_| None }
      | parse_usize => { |v| Some(v) }
    )
);

named!(parse_u64_value<&[u8], Option<u64>>,
    alt!(tag!("unlimited") => { |_| None }
        | parse_u64 => { |v| Some(v) })
);

named!(parse_unit<&[u8], Unit>,
    alt!(
        tag!("seconds") => { |_| Unit::Seconds }
      | tag!("us") => { |_| Unit::Microseconds }
    )
);

/// Parses a usize limit line
named!(parse_usize_line<&[u8], (Option<usize>, Option<usize>)>,
    do_parse!(
        before_soft_value >>
        soft: parse_usize_value >>
        between_soft_hard_value >>
        hard: parse_usize_value >>
        end_of_line >>

        ((soft, hard))
    )
);

/// Parses a u64 limit line
named!(parse_u64_line<&[u8], (Option<u64>, Option<u64>)>,
    do_parse!(
        before_soft_value >>
        soft: parse_u64_value >>
        between_soft_hard_value >>
        hard: parse_u64_value >>
        end_of_line >>

        ((soft, hard))
    )
);

/// Parses a Duration limit line
named!(parse_duration_line<&[u8], (Option<Duration>, Option<Duration>)>,
    do_parse!(
        before_soft_value >>
        soft: parse_u64_value >>
        between_soft_hard_value >>
        hard: parse_u64_value >>
        take_while!(is_space) >>
        unit: parse_unit >>
        end_of_line >>

        ((
            soft.and_then(|v| { get_duration_from_unit(v, &unit) }),
            hard.and_then(|v| { get_duration_from_unit(v, &unit) })
        ))
    )
);

fn get_duration_from_unit(value: u64, unit: &Unit) -> Option<Duration> {
    match unit{
        &Unit::Seconds => Some(Duration::new(value, 0)),
        &Unit::Microseconds => Some(Duration::new(0, value as u32 * 1000))
    }
}

/// Process limits information
/// See man 2 getrlimit
#[derive(Debug, PartialEq, Eq)]
pub struct Limits {
    /// The maximum CPU time a process can use, in seconds
    pub max_cpu_time: (Option<Duration>, Option<Duration>),
    /// The maximum size of files that the process may create
    pub max_file_size: (Option<u64>, Option<u64>),
    /// The maximum size of the process's data segment
    pub max_data_size: (Option<usize>, Option<usize>),
    /// The  maximum size of the process stack
    pub max_stack_size: (Option<usize>, Option<usize>),
    /// Maximum size of a core file
    pub max_core_file_size: (Option<u64>, Option<u64>),
    /// Specifies  the limit of the process's resident set
    pub max_resident_set: (Option<usize>, Option<usize>),
    /// The maximum number of processes (or, more precisely on Linux, threads)
    /// that can be created for the real user ID of the calling process
    pub max_processes: (Option<usize>, Option<usize>),
    ///  Specifies  a value one greater than the maximum file descriptor
    ///  number that can be opened by this process
    pub max_open_files: (Option<usize>, Option<usize>),
    /// The maximum number of bytes of memory that may be locked into RAM
    pub max_locked_memory: (Option<usize>, Option<usize>),
    /// The maximum size of the process's virtual memory (address space)
    pub max_address_space: (Option<usize>, Option<usize>),
    /// A limit on the combined number of locks and leases that this process may
    /// establish
    pub max_file_locks: (Option<usize>, Option<usize>),
    /// Specifies  the  limit  on the number of signals that may be queued for the real user ID of
    /// the calling process
    pub max_pending_signals: (Option<usize>, Option<usize>),
    /// Specifies the limit on the number of bytes that can be allocated for POSIX message queues
    /// for the real user ID of the calling process
    pub max_msgqueue_size: (Option<usize>, Option<usize>),
    /// Specifies  a  ceiling  to  which the process's nice value can be raised
    pub max_nice_priority: (Option<usize>, Option<usize>),
    /// Specifies a limit on the amount of CPU time that a process scheduled
    /// under a real-time scheduling policy may consume without making a blocking
    /// system call
    pub max_realtime_priority: (Option<usize>, Option<usize>),
    /// Specifies a ceiling on the real-time priority that may be set for this process
    pub max_realtime_timeout: (Option<Duration>, Option<Duration>),
}

/// Parses the /proc/<pid>/limits file
fn parse_limits(input: &[u8]) -> IResult<&[u8], Limits> {
    let rest = input;
    let (rest, _)                     = try_parse!(rest, take_until_and_consume!(&b"\n"[..]));
    let (rest, max_cpu_time)          = try_parse!(rest, parse_duration_line);
    let (rest, max_file_size)         = try_parse!(rest, parse_u64_line);
    let (rest, max_data_size)         = try_parse!(rest, parse_usize_line);
    let (rest, max_stack_size)        = try_parse!(rest, parse_usize_line);
    let (rest, max_core_file_size)    = try_parse!(rest, parse_u64_line);
    let (rest, max_resident_set)      = try_parse!(rest, parse_usize_line);
    let (rest, max_processes)         = try_parse!(rest, parse_usize_line);
    let (rest, max_open_files)        = try_parse!(rest, parse_usize_line);
    let (rest, max_locked_memory)     = try_parse!(rest, parse_usize_line);
    let (rest, max_address_space)     = try_parse!(rest, parse_usize_line);
    let (rest, max_file_locks)        = try_parse!(rest, parse_usize_line);
    let (rest, max_pending_signals)   = try_parse!(rest, parse_usize_line);
    let (rest, max_msgqueue_size)     = try_parse!(rest, parse_usize_line);
    let (rest, max_nice_priority)     = try_parse!(rest, parse_usize_line);
    let (rest, max_realtime_priority) = try_parse!(rest, parse_usize_line);
    let (rest, max_realtime_timeout)  = try_parse!(rest, parse_duration_line);

    IResult::Done(rest, Limits {
        max_cpu_time          : max_cpu_time,
        max_file_size         : max_file_size,
        max_data_size         : max_data_size,
        max_stack_size        : max_stack_size,
        max_core_file_size    : max_core_file_size,
        max_resident_set      : max_resident_set,
        max_processes         : max_processes,
        max_open_files        : max_open_files,
        max_locked_memory     : max_locked_memory,
        max_address_space     : max_address_space,
        max_file_locks        : max_file_locks,
        max_pending_signals   : max_pending_signals,
        max_msgqueue_size     : max_msgqueue_size,
        max_nice_priority     : max_nice_priority,
        max_realtime_priority : max_realtime_priority,
        max_realtime_timeout  : max_realtime_timeout
    })
}

fn limits_file(file: &mut File) -> Result<Limits> {
    // Each limit line has a maximum length of 79 chars
    // There are 16 limits as of now (2017-02-20), plus the header
    // 17 * 79 + EOF => 1344
    let mut buf = [0; 1344];
    map_result(parse_limits(try!(read_to_end(file, &mut buf))))
}

pub fn limits(pid: pid_t) -> Result<Limits> {
    limits_file(&mut try!(File::open(&format!("/proc/{}/limits", pid))))
}

pub fn limits_self() -> Result<Limits> {
    limits_file(&mut try!(File::open("/proc/self/limits")))
}

#[cfg(test)]
pub mod tests {
    use std::time::Duration;
    use parsers::tests::unwrap;
    use super::{parse_limits};

    #[test]
    fn test_parse_limits() {
        let text = b"Limit                     Soft Limit           Hard Limit           Units         \n
Max cpu time              10                   60                   seconds       \n
Max file size             unlimited            unlimited            bytes         \n
Max data size             unlimited            unlimited            bytes         \n
Max stack size            8388608              unlimited            bytes         \n
Max core file size        unlimited            unlimited            bytes         \n
Max resident set          unlimited            unlimited            bytes         \n
Max processes             63632                63632                processes     \n
Max open files            1024                 4096                 files         \n
Max locked memory         65536                65536                bytes         \n
Max address space         unlimited            unlimited            bytes         \n
Max file locks            unlimited            unlimited            locks         \n
Max pending signals       63632                63632                signals       \n
Max msgqueue size         819200               819200               bytes         \n
Max nice priority         0                    0                                  \n
Max realtime priority     0                    0                                  \n
Max realtime timeout      500                  unlimited            us            \n";

        let limits = unwrap(parse_limits(text));

        assert_eq!(Some(Duration::new(10, 0)), limits.max_cpu_time.0);
        assert_eq!(Some(Duration::new(60, 0)), limits.max_cpu_time.1);

        assert_eq!(None, limits.max_file_size.0);
        assert_eq!(None, limits.max_file_size.1);

        assert_eq!(None, limits.max_data_size.0);
        assert_eq!(None, limits.max_data_size.1);

        assert_eq!(Some(8388608), limits.max_stack_size.0);
        assert_eq!(None, limits.max_stack_size.1);

        assert_eq!(None, limits.max_core_file_size.0);
        assert_eq!(None, limits.max_core_file_size.1);

        assert_eq!(None, limits.max_resident_set.0);
        assert_eq!(None, limits.max_resident_set.1);

        assert_eq!(Some(63632), limits.max_processes.0);
        assert_eq!(Some(63632), limits.max_processes.1);

        assert_eq!(Some(1024), limits.max_open_files.0);
        assert_eq!(Some(4096), limits.max_open_files.1);

        assert_eq!(Some(65536), limits.max_locked_memory.0);
        assert_eq!(Some(65536), limits.max_locked_memory.1);

        assert_eq!(None, limits.max_address_space.0);
        assert_eq!(None, limits.max_address_space.1);

        assert_eq!(None, limits.max_file_locks.0);
        assert_eq!(None, limits.max_file_locks.1);

        assert_eq!(Some(63632), limits.max_pending_signals.0);
        assert_eq!(Some(63632), limits.max_pending_signals.1);

        assert_eq!(Some(819200), limits.max_msgqueue_size.0);
        assert_eq!(Some(819200), limits.max_msgqueue_size.1);

        assert_eq!(Some(0), limits.max_nice_priority.0);
        assert_eq!(Some(0), limits.max_nice_priority.1);

        assert_eq!(Some(0), limits.max_realtime_priority.0);
        assert_eq!(Some(0), limits.max_realtime_priority.1);

        assert_eq!(Some(Duration::new(0, 500 * 1000)), limits.max_realtime_timeout.0);
        assert_eq!(None, limits.max_realtime_timeout.1);
    }
}
