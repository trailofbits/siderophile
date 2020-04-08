// adapted from rustfilt to be more lightweight and not require another binary (https://github.com/luser/rustfilt)

use cargo::CliResult;
use std::path::PathBuf;
use structopt::StructOpt;
use regex::{Regex, Captures};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::fs::File;

#[derive(StructOpt, Debug)]
pub struct DemangleArgs {
    #[structopt(long = "input-file", value_name = "FILE", parse(from_os_str))]
    /// input callgraph file
    input_mangled: PathBuf,
    #[structopt(long = "output-file", value_name = "FILE", parse(from_os_str))]
    /// input unsafe deps file
    output_demangled: PathBuf,
}

fn do_demangle(input: &mut BufReader<File>, output: &mut BufWriter<File>) -> io::Result<()> {
    let mangled_name_regex: Regex = Regex::new(r"_(ZN|R)[\$\._[:alnum:]]*").unwrap();
    // NOTE: this is actually more efficient than lines(), since it re-uses the buffer
    let mut buf = String::new();
    while input.read_line(&mut buf)? > 0 {
        {
            // NOTE: This includes the line-ending, and leaves it untouched
            let demangled_line = mangled_name_regex.replace_all(&buf, |captures: &Captures| {
                format!("{:#}", rustc_demangle::demangle(&captures[0]))
            });
            // TODO: what do i do with this code?
            if cfg!(debug_assertions) && buf.ends_with('\n') {
                let line_ending = if buf.ends_with("\r\n") { "\r\n" } else { "\n" };
                debug_assert!(demangled_line.ends_with(line_ending), "Demangled line has incorrect line ending");
            }
            output.write_all(demangled_line.as_bytes())?;
        }
        buf.clear(); // Reset the buffer's position, without freeing it's underlying memory
    }
    Ok(()) // Successfully hit EOF
}

pub fn real_main(args: &DemangleArgs) -> CliResult {
    let mut input_reader = BufReader::new(File::open(&args.input_mangled).unwrap());
    let mut output_writer = BufWriter::new(File::create(&args.output_demangled).unwrap());
    do_demangle(&mut input_reader, &mut output_writer).unwrap();
    Ok(())
}
