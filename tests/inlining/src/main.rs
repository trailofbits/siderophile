// This test is there to check if Siderophile can properly taint and detect
// unsafe calls when a given unsafe function can be inlined.
// It also validates if Siderophile works on binary crates.
//
// This issue was mitigated in https://github.com/trailofbits/siderophile/pull/17/files#r352878953 PR
// by removing the `-C inline-threshold=9999` flag from building Siderophile
//
//
use std::io::Cursor;
use byteorder::{BigEndian, ReadBytesExt};

pub fn main() {
    let v = vec![1, 2, 3, 4, 5, 6, 7, 8];
    foobar(v);
}

pub fn foobar(v: Vec<u8>) {
    let mut rdr = Cursor::new(v);
    assert_eq!(0.01, rdr.read_f64::<BigEndian>().unwrap());
}

