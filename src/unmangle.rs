//! Path unmangling functions.
//!
//! Paths included in several files under `/proc` are mangled, i.e., some
//! characters are replaced by the corresponding octal escape sequence `\nnn`.
//!
//! For example, the following files:
//!
//! - `/proc/[pid]/maps`
//! - `/proc/[pid]/smaps`
//! - `/proc/[pid]/numa_maps`
//! - `/proc/swaps`
//!
//! contain mangled paths.
//!
//! This module provides the [`unmangled_path`] function to reverse the
//! mangling (decoding the escape sequences).
//!
//! Note that, unless `\` is included in the set of the escaped characters
//! (which is *not* the case in any of the previous files), the mangling is
//! actually non-reversible (i.e., the demangling is ambiguous).
//!
//! See `mangle_path` in `Linux/fs/seq_file.c` for details on the mangling
//! algorithm.
//!
//! [`unmangled_path`]: fn.unmangled_path.html

use std::str;
use std::num::ParseIntError;

/// Converts a bytes slice in a given base to an integer.
fn u8_from_bytes_radix(bytes: &[u8], radix: u32) -> Result<u8, ParseIntError> {
    let s = unsafe { str::from_utf8_unchecked(bytes) };
    u8::from_str_radix(s, radix)
}

/// Returns a `Vec<u8>` containing the unmangled representation of `path`.
///
/// Octal escape sequences `\nnn` for characters included in `escaped` are
/// decoded.
///
/// This reverses the escaping done by `mangle_path` in `Linux/fs/seq_file.c`.
///
/// # Examples
///
/// To decode only escaped newlines (leaving other escaped sequences alone):
///
/// ```rust,ignore
/// let path = unmangled_path(br"a\012\040path", b"\n");
/// assert_eq!(path, b"a\n\\040path");
/// ```
pub fn unmangled_path(path: &[u8], escaped: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut path = path;
    while let Some((&c, rest)) = path.split_first() {
        if c == b'\\' && rest.len() >= 3 {
            if let Ok(decoded) = u8_from_bytes_radix(&rest[..3], 8) {
                if escaped.contains(&decoded) {
                    result.push(decoded);
                    path = &rest[3..];
                    continue;
                }
            }
        }
        result.push(c);
        path = rest;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unmangle_path() {
        assert_eq!(unmangled_path(b"abcd", b"\n"), b"abcd");
        assert_eq!(unmangled_path(br"a\012path", b"\n"), b"a\npath");
        assert_eq!(unmangled_path(br"a\012\040path", b"\n"), b"a\n\\040path");
        assert_eq!(unmangled_path(br"a\012\040path", b"\n "), b"a\n path");
    }
}
