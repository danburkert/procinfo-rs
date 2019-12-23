use parsers::{map_result, parse_usize};
use nom::{space};

use std::str::{self, FromStr};
use std::io::{Result};
use std::fs;
use std::cmp;
use std::ops::Div;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Cpu {
    /// system ("cpu" line) or the specific CPU ("cpuN" line) spent in various states
    pub cpuid: String,

    /// Time spent in user mode.
    pub user: usize,

    /// Time spent in user mode with low priority (nice).
    pub nice: usize,

    /// Time spent in system mode.
    pub system: usize,

    /// Time spent in the idle task.  This value should be USER_HZ times the second entry in the /proc/uptime pseudo-file.
    pub idle: usize,

    /// Time waiting for I/O to complete.  This
    ///                            value is not reliable, for the following rea‐
    ///                            sons:
    ///
    ///                            1. The CPU will not wait for I/O to complete;
    ///                               iowait is the time that a task is waiting for
    ///                               I/O to complete.  When a CPU goes into idle
    ///                               state for outstanding task I/O, another task
    ///                               will be scheduled on this CPU.
    ///
    ///                            2. On a multi-core CPU, the task waiting for I/O
    ///                               to complete is not running on any CPU, so the
    ///                               iowait of each CPU is difficult to calculate.
    ///
    ///                            3. The value in this field may decrease in cer‐
    ///                               tain conditions.
    pub iowait: usize,

    /// Time servicing interrupts.
    pub irq: usize,

    /// Time servicing softirqs.
    pub softirq: usize,

    /// Stolen time, which is the time spent in
    ///                            other operating systems when running in a virtu‐alized environment
    pub steal: usize,

    /// Time spent running a virtual CPU for guest operating systems
    /// under the control of the Linux kernel
    pub guest: usize,

    /// Time spent running a niced guest
    /// (virtual CPU for guest operating systems
    /// under the control of the Linux kernel)
    pub guest_nice: usize,
}


/// Parses a space-terminated string field in a mountinfo entry
named!(parse_string_field<String>,
       map_res!(map_res!(is_not!(" "), str::from_utf8), FromStr::from_str));

/// Parses a cpu line or cpuN line from /proc/stat.
named!(parse_cpu_info<Cpu>,
    do_parse!(
              cpuid: parse_string_field  >> space >>
              user: parse_usize          >> space >>
              nice: parse_usize          >> space >>
              system: parse_usize        >> space >>
              idle: parse_usize          >> space >>
              iowait: parse_usize        >> space >>
              irq: parse_usize           >> space >>
              softirq: parse_usize       >> space >>
              steal: parse_usize         >> space >>
              guest: parse_usize         >> space >>
              guest_nice: parse_usize    >>
              (Cpu {
                            cpuid: cpuid,
                            user: user,
                            nice: nice,
                            system: system,
                            idle: idle,
                            iowait: iowait,
                            irq: irq,
                            softirq: softirq,
                            steal: steal,
                            guest: guest,
                            guest_nice: guest_nice,
           } )));


/// Returns information about cpu line aggregated statistics.
///
/// Very first line `cpu` aggregates the numbers in all of the other "cpuN" lines in `/proc/stat`.
fn cpu_line_aggregated_entry() -> Result<Cpu> {
    let data = fs::read_to_string("/proc/stat")?;
    let lines: Vec<&str> = data.lines().collect();
    let cpu_line_info = try!(map_result(parse_cpu_info(lines[0].as_bytes())));
    Ok(cpu_line_info)
}

/// Returns the count of the `cpuN lines`.
pub fn cpu_count() -> Result<usize> {
    let data = fs::read_to_string("/proc/stat")?;
    let lines: Vec<&str> = data.lines().collect();
    let mut cpus = 0;
    for line in lines {
        if line.starts_with("cpu") {
            cpus += 1;
        }
    }
    Ok(cmp::max(cpus - 1, 1))
}

pub fn cpu_period() -> Result<usize> {
    let cpu = cpu_line_aggregated_entry().unwrap();
    let total_time = cpu.user + cpu.nice + cpu.system + cpu.irq + cpu.softirq +
                              cpu.idle + cpu.iowait + cpu.steal + cpu.guest + cpu.guest_nice;
    let cpu_count = cpu_count().unwrap();
    Ok(total_time.div(cpu_count))
}


#[cfg(test)]
pub mod tests {
    pub use pid::cpu::{Cpu};
    use pid::cpu::parse_cpu_info;

    /// Test parsing a single mountinfo entry (positive check).
    #[test]
    fn test_parse_cpu_time_info_entry() {
        let entry =
            b"cpu0 49663 0 40234 104757317 542691 4420 39572 0 0 0";
        let got_mi = parse_cpu_info(entry).unwrap().1;
        let want_mi = Cpu {
            cpuid: "cpu0".to_string(),
            user: 49663,
            nice: 0,
            system: 40234,
            idle: 104757317,
            iowait: 542691,
            irq: 4420,
            softirq: 39572,
            steal: 0,
            guest: 0,
            guest_nice : 0,
        };
        assert_eq!(got_mi, want_mi);
    }
}