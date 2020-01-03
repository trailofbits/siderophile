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

