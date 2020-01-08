// This program tests if we can use Siderophile with crates which dependencies
// use the `rust-toolchain` file
// (https://doc.rust-lang.org/stable/edition-guide/rust-2018/rustup-for-managing-rust-versions.html#managing-versions)
// so it effectively tests if https://github.com/trailofbits/siderophile/issues/14 is fixed/handled
// properly
//
// NOTE: It explicitly requires bitvec 0.15.2 as this version uses the `rust-toolchain` file.

////
//// Below is an example output what happens when Siderophle is run on a crate that has
//// `rust-toolchain` file.
////
/*
trawling source code of dependencies for unsafety
    Checking bitvec v0.15.2
    Checking crate-uses-rust-toolchain v0.1.0 (/Users/dc/tob/projects/siderophile/tests/crate-use-rust-toolchain)
error[E0463]: can't find crate for `bitvec`                              ] 1/2: crate-uses-rust-toolchain
 --> src/lib.rs:9:1
  |
9 | extern crate bitvec;
  | ^^^^^^^^^^^^^^^^^^^^ can't find crate

error: aborting due to previous error

For more information about this error, try `rustc --explain E0463`.
thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: Cargo("Could not compile `crate-uses-rust-toolchain`.")', src/libcore/result.rs:1187:5
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace.
*/

extern crate bitvec;

use bitvec::prelude::*;

pub fn foo() {
    let bv = bitvec![BigEndian, u8; 0, 1, 0, 1];

    println!("bv={:?}", bv);
}
