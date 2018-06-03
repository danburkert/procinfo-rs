//! Supported filesystems from `/proc/filesystems`.

use std::fs::File;
use std::io::{BufRead, BufReader, Result};

use nom::tab;

use parsers::{map_result, parse_line};

/// Supported filesystems.
///
/// This is a list of filesystems which which are supported by the
/// kernel, namely filesystems which were compiled into the kernel
/// or whose kernel modules are currently loaded.  If a filesystem
/// is marked with "nodev", this means that it does not require a
/// block device to be mounted (e.g., virtual filesystem, network
/// filesystem).
///
/// See `man 5 proc` and `Linux/fs/filesystems.c`.
#[derive(Debug, Default, PartialEq)]
pub struct Filesystem {
    /// The filesystem does not require a block device to be mounted (e.g., virtual filesytems, network filesystems).
    pub nodev: bool,
    /// The name of the filesystem (e.g. "ext4").
    pub name: String,
}

/// Parses a filesystem entry according to filesystems file format.
named!(parse_filesystem<Filesystem>,
    do_parse!(nodev: opt!(tag!("nodev"))       >> tab >>
              name: parse_line                 >>
              ( Filesystem { nodev: nodev.is_some(), name: name } )));

/// Returns the supported filesystems.
pub fn filesystems() -> Result<Vec<Filesystem>> {
    let mut file = try!(File::open("/proc/filesystems"));
    let mut r = Vec::new();
    for line in BufReader::new(&mut file).lines() {
        let fs = try!(map_result(parse_filesystem(try!(line).as_bytes())));
        r.push(fs);
    }
    Ok(r)
}

#[cfg(test)]
pub mod tests {
    use super::{Filesystem, parse_filesystem, filesystems};

    /// Test parsing a single filesystems entry (positive check).
    #[test]
    fn test_parse_filesystem() {
        let entry =
            b"\text4";
        let got_fs = parse_filesystem(entry).unwrap().1;
        let want_fs = Filesystem {
            nodev: false,
            name: "ext4".to_string(),
        };
        assert_eq!(got_fs, want_fs);
    }

    /// Test parsing a single filesystems entry with nodev (positive check).
    #[test]
    fn test_parse_nodev_filesystem() {
        let entry =
            b"nodev\tfuse";
        let got_fs = parse_filesystem(entry).unwrap().1;
        let want_fs = Filesystem {
            nodev: true,
            name: "fuse".to_string(),
        };
        assert_eq!(got_fs, want_fs);
    }

    /// Test parsing a single filesystem entry (negative check).
    #[test]
    fn test_parse_filesystem_error() {
        let entry = b"garbage";
        parse_filesystem(entry).unwrap_err();
    }

    /// Test that the system filesystems file can be parsed.
    #[test]
    fn test_filesystems() {
        filesystems().unwrap();
    }
}
