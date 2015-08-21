#![recursion_limit = "1000"]
#![cfg_attr(test, feature(test))]

#![allow(dead_code)] // TODO: remove

#[macro_use]
extern crate nom;

extern crate byteorder;

mod parsers;
mod statm;
mod status;

pub use statm::{Statm, statm, statm_self};
pub use status::{SeccompMode, Status, status, status_self};

/// The state of a process.
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum State {
    /// Running.
    Running,
    /// Sleeping in an interruptible wait.
    Sleeping,
    /// Waiting in uninterruptible disk sleep.
    Waiting,
    /// Zombie.
    Zombie,
    /// Stopped (on a signal) or (before Linux 2.6.33) trace stopped.
    Stopped,
    /// trace stopped.
    ///
    /// Linux 2.6.33 onward.
    TraceStopped,
    /// Paging.
    ///
    /// Only before linux 2.6.0.
    Paging,
    /// Dead.
    ///
    /// Linux 2.6.33 to 3.13 only.
    Dead,
    /// Wakekill.
    ///
    /// Linux 2.6.33 to 3.13 only.
    Wakekill,
    /// Waking.
    ///
    /// Linux 2.6.33 to 3.13 only.
    Waking,
    /// Parked.
    ///
    /// Linux 3.9 to 3.13 only.
    Parked,
}

impl Default for State {
    fn default() -> State {
        State::Running
    }
}

/// Parse the stat state format.
named!(parse_stat_state<State>,
       alt!(tag!("R") => { |_| State::Running  }
          | tag!("S") => { |_| State::Sleeping }
          | tag!("D") => { |_| State::Waiting }
          | tag!("Z") => { |_| State::Zombie }
          | tag!("T") => { |_| State::Stopped }
          | tag!("t") => { |_| State::TraceStopped }
          | tag!("W") => { |_| State::Paging }
          | tag!("X") => { |_| State::Dead }
          | tag!("x") => { |_| State::Dead }
          | tag!("K") => { |_| State::Wakekill }
          | tag!("W") => { |_| State::Waking }
          | tag!("P") => { |_| State::Parked }));
