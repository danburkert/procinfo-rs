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

/// Null device number (anonymous mappings).
const NULL_DEV: libc::dev_t = 0;

/// File-backed memory mapping.
///
/// # Notes
///
/// Due to the way paths are encoded by the kernel before exposing them in
/// `/proc/[pid]/maps`, the parsing of `path` and `is_deleted` is
/// ambiguous. For example, all the following path/deleted combinations:
///
/// - `"/tmp/a\nfile"` *(deleted file)*
/// - `"/tmp/a\nfile (deleted)"` *(existing file)*
/// - `"/tmp/a\\012file"` *(deleted file)*
/// - `"/tmp/a\\012file (deleted)"` *(existing file)*
///
/// will be mangled by the kernel, resulting in the same `pathname`
/// value (`"/tmp/a\\012file (deleted)"`), and will be decoded by this
/// library as:
///
/// ```rust,ignore
/// FileMap {
///    ...,
///    path: PathBuf::from("/tmp/a\nfile"),
///    is_deleted: true,
/// }
/// ```
///
/// If the `path` of a mapping is required for other than purely informational
/// uses (such as opening and/or memory mapping it), a more reliable source
/// (such as `/proc/[pid]/map_files`) should be used, if available. The open
/// file `(dev, inode)` should also be checked against the values provided by
/// the mapping.
///
/// See `Linux/fs/seq_file.c`, and `Linux/fs/dcache.c`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileMap {
    /// Offset into the file backing this mapping.
    pub offset: u64,
    /// Device containing the file backing this mapping.
    pub dev: libc::dev_t,
    /// Inode of the file backing this mapping.
    pub inode: libc::ino_t,
    /// Path to the file backing this mapping.
    pub path: PathBuf,
    /// Whether the file backing this mapping has been deleted.
    pub is_deleted: bool,
}

/// Memory mapping kind.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemoryMapKind {
    /// Anonymous memory mapping.
    Anonymous,
    /// File-backed memory mapping.
    File(FileMap),
    /// Anonymous memory mapping containing the process heap.
    Heap,
    /// Anonymous memory mapping containing the process stack.
    Stack,
    /// Unknown anonymous memory mapping.
    Unknown(String),
    /// Kernel mapping for vDSO code.
    Vdso,
    /// Pseudo-mapping providing access to the kernel `vsyscall` page.
    Vsyscall,
    /// Kernel mapping for vDSO data pages.
    Vvar,
}

/// Process memory mapping information.
///
/// See `man 5 proc` and `Linux/fs/proc/task_mmu.c`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MemoryMap {
    /// Address range that this mapping occupies in the process virtual memory
    /// space.
    pub range: ops::Range<usize>,
    /// Whether pages in this mapping may be read.
    pub is_readable: bool,
    /// Whether pages in this mapping may be written.
    pub is_writable: bool,
    /// Whether pages in this mapping may be executed.
    pub is_executable: bool,
    /// Whether this mapping is shared or private.
    pub is_shared: bool,
    /// Mapping type (file-backed or anonymous).
    pub kind: MemoryMapKind,
}

impl MemoryMap {
    /// Returns file mapping information for file-backed mappings.
    pub fn file(&self) -> Option<&FileMap> {
        match self.kind {
            MemoryMapKind::File(ref file_map) => Some(file_map),
            _ => None,
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

/// Parses device number.
named!(parse_dev<&[u8], libc::dev_t>, do_parse!(
    major: parse_u32_hex >> tag!(":") >> minor: parse_u32_hex >>
    (unsafe {libc::makedev(major, minor)})
));

/// Parses `pathname` field for anonymous mappings.
fn parse_anonymous_pathname(bytes: &[u8]) -> MemoryMapKind {
    if bytes.is_empty() {
        MemoryMapKind::Anonymous
    } else if bytes == b"[heap]" {
        MemoryMapKind::Heap
    } else if bytes.starts_with(b"[stack") {
        MemoryMapKind::Stack
    } else if bytes == b"[vdso]" {
        MemoryMapKind::Vdso
    } else if bytes == b"[vsyscall]" {
        MemoryMapKind::Vsyscall
    } else if bytes == b"[vvar]" {
        MemoryMapKind::Vvar
    } else {
        MemoryMapKind::Unknown(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// Truncates byte vector removing a suffix if present.
fn truncate_suffix(bytes: &mut Vec<u8>, suffix: &[u8]) -> bool {
    if bytes.ends_with(suffix) {
        let length = bytes.len() - suffix.len();
        bytes.truncate(length);
        true
    } else {
        false
    }
}

/// Parses `pathname` field for file-backed mappings.
fn parse_file_pathname(bytes: &[u8]) -> (PathBuf, bool) {
    let mut path = unmangled_path(bytes, b"\n");
    let is_deleted = truncate_suffix(&mut path, b" (deleted)");
    (PathBuf::from(OsString::from_vec(path)), is_deleted)
}

/// Parses a maps entry.
named!(parse_maps_entry<&[u8], MemoryMap>, do_parse!(
    start: parse_usize_hex >> tag!("-") >>
    end: parse_usize_hex >> space >>
    is_readable: perms_read >>
    is_writable: perms_write >>
    is_executable: perms_execute >>
    is_shared: perms_shared >> space >>
    offset: parse_u64_hex >> space >>
    dev: parse_dev >> space >>
    inode: parse_u64 >> space >>
    pathname: rest >>
    (MemoryMap {
        range: ops::Range{start: start, end: end},
        is_readable: is_readable,
        is_writable: is_writable,
        is_executable: is_executable,
        is_shared: is_shared,
        kind: match dev {
            NULL_DEV => parse_anonymous_pathname(pathname),
            _ => {
                let (path, is_deleted) = parse_file_pathname(pathname);
                MemoryMapKind::File(FileMap{
                    offset: offset,
                    dev: dev,
                    inode: inode,
                    path: path,
                    is_deleted: is_deleted,
                })
            }
        },
    })
));

/// Parses the provided maps file.
fn maps_file<R: io::Read>(file: &mut R) -> io::Result<Vec<MemoryMap>> {
    io::BufReader::new(file)
        .split(b'\n')
        .map(|line| map_result(parse_maps_entry(&line?)))
        .collect()
}

/// Returns mapped memory regions information for the process with the provided
/// pid.
pub fn maps(pid: libc::pid_t) -> io::Result<Vec<MemoryMap>> {
    maps_file(&mut fs::File::open(format!("/proc/{}/maps", pid))?)
}

/// Returns mapped memory regions information for the current process.
pub fn maps_self() -> io::Result<Vec<MemoryMap>> {
    maps_file(&mut fs::File::open("/proc/self/maps")?)
}

#[cfg(test)]
pub mod tests {
    use libc;
    use std::path::Path;
    use std::io;
    use super::*;
    use super::{maps_file, parse_file_pathname, parse_maps_entry};

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
        assert_eq!(Path::new("/bin/cat\r"), maps[0].file().unwrap().path);
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

        let file_map = map.file().unwrap();
        assert_eq!(0, file_map.offset);
        assert_eq!(unsafe { libc::makedev(0xfd, 0x1) }, file_map.dev);
        assert_eq!(8650756, file_map.inode);
        assert_eq!(Path::new("/bin/cat"), file_map.path);
        assert!(!file_map.is_deleted);
    }

    #[test]
    fn test_parse_maps_entry_no_path() {
        let maps_entry_text = b"\
7f8ec1d99000-7f8ec1dbe000 rw-p 00000000 00:00 0 ";

        let map = parse_maps_entry(maps_entry_text).to_result().unwrap();
        assert_eq!(map.kind, MemoryMapKind::Anonymous);
    }

    #[test]
    fn test_parse_file_pathname() {
        assert_eq!(
            parse_file_pathname(b"/bin/cat"),
            (Path::new("/bin/cat").to_owned(), false)
        );

        assert_eq!(
            parse_file_pathname(b"/bin/cat (deleted)"),
            (Path::new("/bin/cat").to_owned(), true)
        );

        assert_eq!(
            parse_file_pathname(br"/bin/a program"),
            (Path::new("/bin/a program").to_owned(), false)
        );

        assert_eq!(
            parse_file_pathname(br"/bin/a\012program"),
            (Path::new("/bin/a\nprogram").to_owned(), false)
        );
    }
}
