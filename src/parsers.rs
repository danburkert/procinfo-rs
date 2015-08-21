//! Parsers and utility functions.

use std::io::{Error, ErrorKind, Read, Result};
use std::str::{self, FromStr};
use std::fs::File;

use byteorder::{ByteOrder, LittleEndian};
use nom::{alphanumeric, digit, is_digit, not_line_ending, space, IResult};

/// Read all bytes in the file until EOF, placing them into `buf`.
///
/// All bytes read from this source will be written to `buf`.  If `buf` is not large enough an
/// underflow error will be returned. This function will continuously call `read` to append more
/// data to `buf` until read returns either `Ok(0)`, or an error of non-`ErrorKind::Interrupted`
/// kind.
///
/// If successful, this function will return the slice of read bytes.
///
/// # Errors
///
/// If this function encounters an error of the kind `ErrorKind::Interrupted` then the error is
/// ignored and the operation will continue.
///
/// If any other read error is encountered then this function immediately returns.  Any bytes which
/// have already been read will be written to `buf`.
///
/// If `buf` is not large enough to hold the file, an underflow error will be returned.
pub fn read_to_end<'a>(file: &mut File, buf: &'a mut [u8]) -> Result<&'a mut [u8]> {
    let mut from = 0;

    loop {
        if from == buf.len() {
            return Err(Error::new(ErrorKind::Other, "read underflow"));
        }
        match file.read(&mut buf[from..]) {
            Ok(0) => return Ok(&mut buf[..from]),
            Ok(n) => from += n,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

fn fdigit(input:&[u8]) -> IResult<&[u8], &[u8]> {
  for idx in 0..input.len() {
    if (!is_digit(input[idx])) && ('.' as u8 != input[idx]) {
      return IResult::Done(&input[idx..], &input[0..idx])
    }
  }
  IResult::Done(b"", input)
}
/// Parses the remainder of the line to a string.
named!(pub parse_to_end<String>,
       map_res!(map_res!(not_line_ending, str::from_utf8), FromStr::from_str));

/// Parses a u32 in base-10 format.
named!(pub parse_u32<u32>,
       map_res!(map_res!(digit, str::from_utf8), FromStr::from_str));

/// Parses a f32 in base-10 format.
named!(pub parse_f32<f32>,
      map_res!(map_res!(fdigit, str::from_utf8), FromStr::from_str));

/// Parses a sequence of whitespace seperated u32s.
named!(pub parse_u32s<Vec<u32> >, separated_list!(space, parse_u32));

/// Parses an i32 in base-10 format.
named!(pub parse_i32<i32>, map!(parse_u32, { |n| { n as i32 } }));

/// Parses a sequence of whitespace seperated i32s.
named!(pub parse_i32s<Vec<i32> >, separated_list!(space, parse_i32));

/// Parses a u64 in base-10 format.
named!(pub parse_u64<u64>,
       map_res!(map_res!(digit, str::from_utf8), FromStr::from_str));

/// Parses a usize in base-10 format.
named!(pub parse_usize<usize>,
       map_res!(map_res!(digit, str::from_utf8), FromStr::from_str));

/// Parses a usize followed by a kB unit tag.
named!(pub parse_kb<usize>,
       chain!(space ~ bytes: parse_usize ~ space ~ tag!("kB"), || { bytes }));

/// Parses a u32 in base-16 format.
named!(pub parse_u32_hex<u32>,
       map_res!(map_res!(alphanumeric, str::from_utf8),
                |s| u32::from_str_radix(s, 16)));

/// Parses a u64 in base-16 format.
named!(pub parse_u64_hex<u64>,
       map_res!(map_res!(alphanumeric, str::from_utf8),
                |s| u64::from_str_radix(s, 16)));

/// Reverses the bits in a byte.
fn reverse(n: u8) -> u8 {
    // stackoverflow.com/questions/2602823/in-c-c-whats-the-simplest-way-to-reverse-the-order-of-bits-in-a-byte
    const LOOKUP: [u8; 16] = [ 0x0, 0x8, 0x4, 0xc, 0x2, 0xa, 0x6, 0xe,
                               0x1, 0x9, 0x5, 0xd, 0x3, 0xb, 0x7, 0xf ];
    (LOOKUP[(n & 0b1111) as usize] << 4) | LOOKUP[(n >> 4) as usize]
}

/// Parses a list of u32 masks into an array of bytes in `BitVec` format.
///
/// See cpuset(7) for the format being parsed.
named!(pub parse_u32_mask_list<Box<[u8]> >,
       map!(separated_nonempty_list!(tag!(","), parse_u32_hex), |mut ints: Vec<u32>| {
           let mut bytes: Vec<u8> = Vec::with_capacity(ints.len() * 4);
           let mut buf: [u8; 4] = [0; 4];
           ints.reverse();
           for int in ints {
               LittleEndian::write_u32(&mut buf, int);
               for b in buf.iter_mut() {
                   *b = reverse(*b);
               }
               bytes.extend(&buf);
           }
           bytes.into_boxed_slice()
       }));

#[cfg(test)]
mod tests {

    extern crate test;

    use std::u32;

    use nom::IResult;

    use super::{parse_i32s, parse_u32_hex, parse_u32_mask_list, parse_u32s, reverse};

    fn unwrap<'a, I, O>(result: IResult<'a, I, O>) -> O {
        match result {
            IResult::Done(_, val) => val,
            _ => panic!("unable to unwrap IResult"),
        }
    }

    #[test]
    fn test_reverse() {
        assert_eq!(0b00000000, reverse(0b00000000));
        assert_eq!(0b00000010, reverse(0b01000000));
        assert_eq!(0b00011000, reverse(0b00011000));
        assert_eq!(0b01011000, reverse(0b00011010));
        assert_eq!(0b11111111, reverse(0b11111111));
    }

    #[test]
    fn test_parse_u32_hex() {
        assert_eq!(0, unwrap(parse_u32_hex(b"00000000")));
        assert_eq!(1, unwrap(parse_u32_hex(b"00000001")));
        assert_eq!(42, unwrap(parse_u32_hex(b"0000002a")));
        assert_eq!(286331153, unwrap(parse_u32_hex(b"11111111")));
        assert_eq!(u32::MAX, unwrap(parse_u32_hex(b"ffffffff")));
    }

    #[test]
    fn test_u32_mask_list() {
        // Examples adapted from cpuset(7).
        assert_eq!([0, 0, 0, 0], &*unwrap(parse_u32_mask_list(b"00000000")));

        assert_eq!([0x80, 0, 0, 0], &*unwrap(parse_u32_mask_list(b"00000001")));

        assert_eq!([0, 0, 0, 0,
                    0, 0, 0, 0,
                    0, 0, 0, 2], &*unwrap(parse_u32_mask_list(b"40000000,00000000,00000000")));

        assert_eq!([0, 0, 0, 0,
                    0, 0, 0, 0,
                    128, 0, 0, 0], &*unwrap(parse_u32_mask_list(b"00000001,00000000,00000000")));

        assert_eq!([0, 0, 0, 0,
                    0xff, 0, 0, 0], &*unwrap(parse_u32_mask_list(b"000000ff,00000000")));

        assert_eq!([0x46, 0x1c, 0x70, 0,
                    0, 0, 0, 0], &*unwrap(parse_u32_mask_list(b"00000000,000e3862")));
    }

    #[test]
    fn test_parse_u32s() {
        assert_eq!(Vec::<u32>::new(), &*unwrap(parse_u32s(b"")));
        assert_eq!(vec![0u32], &*unwrap(parse_u32s(b"0")));
        assert_eq!(vec![0u32, 1], &*unwrap(parse_u32s(b"0 1")));
        assert_eq!(vec![99999u32, 32, 22, 888], &*unwrap(parse_u32s(b"99999 32 22 	888")));
    }
    #[test]
    fn test_parse_i32s() {
        assert_eq!(Vec::<i32>::new(), &*unwrap(parse_i32s(b"")));
        assert_eq!(vec![0i32], &*unwrap(parse_i32s(b"0")));
        assert_eq!(vec![0i32, 1], &*unwrap(parse_i32s(b"0 1")));
        assert_eq!(vec![99999i32, 32, 22, 888], &*unwrap(parse_i32s(b"99999 32 22 	888")));
    }
}
