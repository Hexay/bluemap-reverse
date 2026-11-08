use super::*;

#[test]
fn little_endian_scalars() {
    let buf = [0xab, 0x01, 0x02, 0x03, 0xfe, 0xff, 0xff, 0xff];
    let mut r = Reader::new(&buf);
    assert_eq!(r.u8().unwrap(), 0xab);
    assert_eq!(r.u24().unwrap(), 0x030201);
    assert_eq!(r.i32().unwrap(), -2);
    assert!(r.at_end());
}

#[test]
fn cstr_consumes_terminator() {
    let mut r = Reader::new(b"uv\0ao\0x");
    assert_eq!(r.cstr().unwrap(), "uv");
    assert_eq!(r.cstr().unwrap(), "ao");
    assert_eq!(r.u8().unwrap(), b'x');
    assert!(r.at_end());
}

#[test]
fn align4_is_relative_to_buffer_start() {
    let buf = [0u8; 12];
    let mut r = Reader::new(&buf);
    r.align4();
    assert_eq!(r.bytes(0).unwrap().len(), 0);
    r.u8().unwrap();
    r.align4();
    assert_eq!(r.pos, 4);
    r.align4();
    assert_eq!(r.pos, 4);
    r.bytes(5).unwrap();
    r.align4();
    assert_eq!(r.pos, 12);
}

#[test]
fn short_reads_error() {
    assert!(Reader::new(&[1, 2]).u24().is_err());
    assert!(Reader::new(&[1, 2, 3]).i32().is_err());
    assert!(Reader::new(&[]).u8().is_err());
    let mut r = Reader::new(&[1]);
    r.u8().unwrap();
    assert!(r.bytes(usize::MAX).is_err(), "length overflow must not wrap");
}

#[test]
fn failed_read_does_not_advance() {
    let mut r = Reader::new(&[7, 8]);
    assert!(r.i32().is_err());
    assert_eq!(r.u8().unwrap(), 7);
}

#[test]
fn bad_strings_error() {
    assert!(Reader::new(b"no terminator").cstr().is_err());
    assert!(Reader::new(&[0xff, 0xfe, 0]).cstr().is_err());
}

#[test]
fn read_after_align_past_end_errors() {
    let mut r = Reader::new(&[1, 2, 3, 4, 5]);
    r.u8().unwrap();
    r.bytes(4).unwrap();
    r.align4();
    assert!(r.at_end());
    assert!(r.bytes(0).is_err());
}

#[test]
fn cstr_after_align_past_end_errors() {
    let mut r = Reader::new(&[1]);
    r.u8().unwrap();
    r.align4();
    assert!(r.cstr().is_err());
}
