//! Process initial environment from `/proc/[pid]/environ`.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::iter::Iterator;

use libc::pid_t;
use nom::{self, IResult};

/// An environment holder.
///
/// Use the `into_iter` method to access environment variables as key-value pairs.
#[derive(Debug, Clone)]
pub struct Environ {
    data: Vec<u8>,
}

/// A lazy iterator over environment variables.
pub struct EnvironIter<'a> {
    data_pointer: &'a [u8],
}

impl<'a> Iterator for EnvironIter<'a> {
    /// Since the data is parsed on the fly, a parsing error could be encountered, hence using an
    /// `io::Result` as an iterator item.
    type Item = Result<(&'a OsStr, &'a OsStr)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data_pointer.is_empty() {
            return None;
        }
        match parse_pair(self.data_pointer) {
            IResult::Done(data, parsed) => {
                self.data_pointer = data;
                Some(Ok(parsed))
            }
            IResult::Incomplete(_) => None,
            IResult::Error(err) => Some(Err(Error::new(
                ErrorKind::InvalidInput,
                format!("Unable to parse input: {:?}", err),
            ))),
        }
    }
}

impl<'a> IntoIterator for &'a Environ {
    type Item = Result<(&'a OsStr, &'a OsStr)>;
    type IntoIter = EnvironIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        EnvironIter {
            data_pointer: &self.data,
        }
    }
}

/// Extracts name of a variable. Also consumes a delimiter.
fn get_name(src: &[u8]) -> IResult<&[u8], &OsStr> {
    // Calculate position of the *equal* sign.
    let pos = match src.iter().skip(1).position(|c| c == &b'=') {
        Some(p) => p,
        None => return IResult::Error(error_position!(nom::ErrorKind::Custom(0), src)),
    };
    IResult::Done(&src[pos + 2..], from_bytes(&src[..pos + 1]))
}

/// Parses "key=value" pair.
named!(
    parse_pair<&[u8], (&OsStr, &OsStr)>,
    tuple!(get_name, map!(take_until_and_consume!("\0"), from_bytes))
);

/// A helper function to convert a slice of bytes to an `OsString`.
fn from_bytes(s: &[u8]) -> &OsStr {
    OsStr::from_bytes(s)
}

/// Parses the provided environ file.
fn environ_path<P: AsRef<Path>>(path: P) -> Result<Environ> {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(Environ { data: buf })
}

/// Returns initial environment for the process with the provided pid as key-value pairs.
pub fn environ(pid: pid_t) -> Result<Environ> {
    environ_path(format!("/proc/{}/environ", pid))
}

/// Returns initial environment for the current process as key-value pairs.
pub fn environ_self() -> Result<Environ> {
    environ_path("/proc/self/environ")
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_get_name() {
        let src = &b"FOO=BAR"[..];
        assert_eq!(get_name(src), IResult::Done(&b"BAR"[..], OsStr::new("FOO")));
        let src = &b"FOO="[..];
        assert_eq!(get_name(src), IResult::Done(&b""[..], OsStr::new("FOO")));
        let src = &b"=FOO=BAR"[..];
        assert_eq!(
            get_name(src),
            IResult::Done(&b"BAR"[..], OsStr::new("=FOO"))
        );
        let src = &b"=FOO="[..];
        assert_eq!(get_name(src), IResult::Done(&b""[..], OsStr::new("=FOO")));
    }

    #[test]
    fn test_pair() {
        let source = &b"FOO=BAR\0123"[..];
        assert_eq!(
            parse_pair(source),
            IResult::Done(&b"123"[..], (OsStr::new("FOO"), OsStr::new("BAR")))
        );
        let source = &b"FOO=\0123"[..];
        assert_eq!(
            parse_pair(source),
            IResult::Done(&b"123"[..], (OsStr::new("FOO"), OsStr::new("")))
        );
        let source = &b"=FOO=BAR\0-"[..];
        assert_eq!(
            parse_pair(source),
            IResult::Done(&b"-"[..], (OsStr::new("=FOO"), OsStr::new("BAR")))
        );
        let source = &b"=FOO=\0-"[..];
        assert_eq!(
            parse_pair(source),
            IResult::Done(&b"-"[..], (OsStr::new("=FOO"), OsStr::new("")))
        );
    }

    #[test]
    fn test_iter() {
        let env = Environ {
            data: b"key1=val1\0=key2=val 2\0key3=val3\0".to_vec(),
        };
        // Here's how you convert the env into a vector.
        let pairs_vec: Result<Vec<(&OsStr, &OsStr)>> = env.into_iter().collect();
        let pairs_vec = match pairs_vec {
            Err(e) => panic!("Parsing has failed: {:?}", e),
            Ok(pairs) => pairs,
        };
        assert_eq!(
            pairs_vec,
            vec![
                (OsStr::new("key1"), OsStr::new("val1")),
                (OsStr::new("=key2"), OsStr::new("val 2")),
                (OsStr::new("key3"), OsStr::new("val3")),
            ]
        );
        // And here's how you create a map.
        let pairs_map: Result<BTreeMap<&OsStr, &OsStr>> = env.into_iter().collect();
        let pairs_map = match pairs_map {
            Err(e) => panic!("Parsing has failed: {:?}", e),
            Ok(pairs) => pairs,
        };
        assert_eq!(pairs_map.get(OsStr::new("key1")), Some(&OsStr::new("val1")));
        assert_eq!(
            pairs_map.get(OsStr::new("=key2")),
            Some(&OsStr::new("val 2"))
        );
        assert_eq!(pairs_map.get(OsStr::new("key3")), Some(&OsStr::new("val3")));
    }

    #[test]
    fn test_environ_self() {
        let env = environ_self().unwrap();
        assert!(env.into_iter().all(|x| x.is_ok()));
    }
}
