//! Process memory mappings information from `/proc/[pid]/maps`.

use std::ffi::OsString;
use std::io::{self, BufRead};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::{fs, ops};

use libc;
use nom::{rest, space};

use parsers::{map_result, parse_usize_hex, parse_u32_hex, parse_u64, parse_u64_hex};
use unmangle::unmangled_path;

/// Process memory mapping information.
///
/// Due to the way paths are encoded by the kernel before exposing them in
/// `/proc/[pid]/maps`, the parsing of `path` and `is_deleted` is
/// ambiguous. For example, all the following path/deleted combinations:
///
/// - `/tmp/a\nfile` *(deleted file)*
/// - `/tmp/a\nfile (deleted)` *(existing file)*
/// - `/tmp/a\\012file` *(deleted file)*
/// - `/tmp/a\\012file (deleted)` *(existing file)*
///
/// will be mangled by the kernel and decoded by this module as:
///
/// ```rust,ignore
/// MemoryMapping (
///    ...,
///    path: PathBuf::from("/tmp/a\nfile"),
///    is_deleted:true,
/// )
/// ```
///
/// If the `path` of a mapping is required for other than purely informational
/// uses (such as opening and/or memory mapping it), a more reliable source
/// (such as `/proc/[pid]/map_files`) should be used, if available. The open
/// file `(dev, inode)` should also be checked against the values provided by
/// the mapping.
///
/// See `man 5 proc`, `Linux/fs/proc/task_mmu.c`, and `Linux/fs/seq_file.c`.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct MemoryMapping {
    /// Address range that this mapping occupies in the process virtual memory
    /// space.
    pub range: ops::Range<usize>,
    /// Whether pages in this mapping may be read.
    pub is_readable: bool,
    /// Whether pages in this mapping may be written.
    pub is_writable: bool,
    /// Whether pages in this mapping may be executed.
    pub is_executable: bool,
    /// Whether this mapping is shared.
    pub is_shared: bool,
    /// Offset into the file backing this mapping (for non-anonymous mappings).
    pub offset: u64,
    /// Device containing the file backing this mapping (for non-anonymous
    /// mappings).
    pub dev: libc::dev_t,
    /// Inode of the file backing this mapping (for non-anonymous mappings).
    pub inode: libc::ino_t,
    /// Path to the file backing this mapping (for non-anonymous mappings),
    /// pseudo-path (such as `[stack]`, `[heap]`, or `[vdso]`) for some special
    /// anonymous mappings, or empty path for other anonymous mappings.
    pub path: PathBuf,
    /// Whether the file backing this mapping has been deleted (for
    /// non-anonymous mappings).
    pub is_deleted: bool,
}

impl MemoryMapping {
    /// Returns `true` if this is an anonymous mapping.
    pub fn is_anonymous(&self) -> bool {
        self.inode == 0
    }
}

/// Parsed `pathname` field.
#[derive(Debug, PartialEq, Eq, Hash)]
struct Pathname {
    path: PathBuf,
    is_deleted: bool,
}

impl Pathname {
    /// Parses a `pathname` field.
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut path = unmangled_path(bytes, b"\n");
        let deleted_suffix = b" (deleted)";
        let is_deleted = path.ends_with(deleted_suffix);
        if is_deleted {
            let length = path.len() - deleted_suffix.len();
            path.truncate(length);
        }
        Pathname {
            path: PathBuf::from(OsString::from_vec(path)),
            is_deleted,
        }
    }
}

/// Parses read permission flag.
named!(perms_read<&[u8], bool>, map!(one_of!("r-"), |c| c == 'r'));

/// Parses write permission flag.
named!(perms_write<&[u8], bool>, map!(one_of!("w-"), |c| c == 'w'));

/// Parses execute permission flag.
named!(perms_execute<&[u8], bool>, map!(one_of!("x-"), |c| c == 'x'));

/// Parses shared/private permission flag.
named!(perms_shared<&[u8], bool>, map!(one_of!("sp"), |c| c == 's'));

/// Parses a maps entry.
named!(parse_maps_entry<&[u8], MemoryMapping>, do_parse!(
    start: parse_usize_hex >> tag!("-") >>
    end: parse_usize_hex >> space >>
    is_readable: perms_read >>
    is_writable: perms_write >>
    is_executable: perms_execute >>
    is_shared: perms_shared >> space >>
    offset: parse_u64_hex >> space >>
    major: parse_u32_hex >> tag!(":") >>
    minor: parse_u32_hex >> space >>
    inode: parse_u64 >> space >>
    pathname: map!(rest, Pathname::from_bytes) >>
    (MemoryMapping {
        range: ops::Range{start, end},
        is_readable,
        is_writable,
        is_executable,
        is_shared,
        offset,
        dev: unsafe {libc::makedev(major, minor)},
        inode,
        path: pathname.path,
        is_deleted: pathname.is_deleted,
    })
));

/// Parses the provided maps file.
fn maps_file<R: io::Read>(file: &mut R) -> io::Result<Vec<MemoryMapping>> {
    io::BufReader::new(file)
        .split(b'\n')
        .map(|line| map_result(parse_maps_entry(&line?)))
        .collect()
}

/// Returns mapped memory regions information for the process with the provided
/// pid.
pub fn maps(pid: libc::pid_t) -> io::Result<Vec<MemoryMapping>> {
    maps_file(&mut fs::File::open(format!("/proc/{}/maps", pid))?)
}

/// Returns mapped memory regions information for the current process.
pub fn maps_self() -> io::Result<Vec<MemoryMapping>> {
    maps_file(&mut fs::File::open("/proc/self/maps")?)
}

#[cfg(test)]
pub mod tests {
    use std::path::Path;

    use super::*;

    /// Test that the current process maps file can be parsed.
    #[test]
    fn test_maps() {
        maps_self().unwrap();
    }

    #[test]
    fn test_maps_file() {
        let maps_text = b"\
5643a788f000-5643a7897000 r-xp 00000000 fd:01 8650756      /bin/cat
7f0540a43000-7f0540a47000 rw-p 00000000 00:00 0 \n";
        let mut buf = io::Cursor::new(maps_text.as_ref());
        let maps = maps_file(&mut buf).unwrap();
        assert_eq!(2, maps.len());
    }

    #[test]
    fn test_maps_file_pathname_ends_with_cr() {
        let maps_text = b"\
5643a788f000-5643a7897000 r-xp 00000000 fd:01 8650756      /bin/cat\r
5643a9412000-5643a9433000 rw-p 00000000 00:00 0            [heap]
";
        let mut buf = io::Cursor::new(maps_text.as_ref());
        let maps = maps_file(&mut buf).unwrap();
        assert_eq!(2, maps.len());
        assert_eq!(Path::new("/bin/cat\r"), maps[0].path);
    }

    #[test]
    fn test_parse_maps_entry() {
        let maps_entry_text = b"\
5643a788f000-5643a7897000 r-xp 00000000 fd:01 8650756      /bin/cat";

        let map = parse_maps_entry(maps_entry_text).to_result().unwrap();
        assert_eq!(0x5643a788f000, map.range.start);
        assert_eq!(0x5643a7897000, map.range.end);
        assert!(map.is_readable);
        assert!(!map.is_writable);
        assert!(map.is_executable);
        assert!(!map.is_shared);
        assert_eq!(0, map.offset);
        assert_eq!(unsafe { libc::makedev(0xfd, 0x1) }, map.dev);
        assert_eq!(8650756, map.inode);
        assert_eq!(Path::new("/bin/cat"), map.path);
        assert!(!map.is_deleted);
    }

    #[test]
    fn test_parse_maps_entry_no_path() {
        let maps_entry_text = b"\
7f8ec1d99000-7f8ec1dbe000 rw-p 00000000 00:00 0 ";

        let map = parse_maps_entry(maps_entry_text).to_result().unwrap();
        assert_eq!(Path::new(""), map.path);
        assert!(!map.is_deleted);
        assert!(map.is_anonymous());
    }

    #[test]
    fn test_pathname_from_bytes() {
        let pathname = Pathname::from_bytes(b"/bin/cat");
        assert_eq!(Path::new("/bin/cat"), pathname.path);
        assert!(!pathname.is_deleted);

        let pathname = Pathname::from_bytes(b"/bin/cat (deleted)");
        assert_eq!(Path::new("/bin/cat"), pathname.path);
        assert!(pathname.is_deleted);

        let pathname = Pathname::from_bytes(br"/bin/a program");
        assert_eq!(Path::new("/bin/a program"), pathname.path);
        assert!(!pathname.is_deleted);

        let pathname = Pathname::from_bytes(br"/bin/a\012program");
        assert_eq!(Path::new("/bin/a\nprogram"), pathname.path);
        assert!(!pathname.is_deleted);
    }
}
