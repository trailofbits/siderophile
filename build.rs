use std::process::exit;

use version_check::Version;

fn main() {
    if !version_check::is_max_version("1.67.1").unwrap_or(false) {
        eprintln!("Rust 1.68 and newer are not supported yet, as they require LLVM 16 or newer.");
        eprintln!("You have: {}", Version::read().unwrap());
        eprintln!(
            "For more information, see https://github.com/trailofbits/siderophile/issues/267"
        );
        exit(1)
    }
}
