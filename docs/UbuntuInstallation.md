# Installing Siderophile on a fresh installation of Ubuntu
This page documents the installation process of Siderophile on a fresh installation of Ubuntu.
There are two methods to install (crates.io or Github) and both are documented below.
## Prerequisites
1. Install curl if its not installed : `sudo apt install curl`
2. Install Rust; follow instructions at: [The Official Rust Documentation](https://doc.rust-lang.org/book/ch01-01-installation.html)
3. Install a compiler linker like GCC or Clang: `sudo apt install build-essential`

## Installing using crates.io
1. Install Siderophile using the command : `cargo install siderophile`
2. You may get an error while compiling openssl-sys : `error: failed to run custom build command for openssl-sys v0.9.102`
3. You can fix the above error by installing the libssl-dev package : `sudo apt install pkg-config libssl-dev`
4. Reload the profile : `. ~/.profile`
5. Re-issue the cargo install command: `cargo install siderophile`
6. You may get an error as below:
```
error: No suitable version of LLVM was found system-wide or pointed
              to by LLVM_SYS_140_PREFIX.
```
7. To fix this you can install the missing version, the version is the number in the error text above eg. 140 or perhaps 170. In the above instance it will be: `sudo apt install llvm-14`
8. You may get an error about missing library `Polly` as below
```
error: could not find native static library `Polly`, perhaps an -L flag is missing?
```
9. To fix this error you can install the missing package(once agian taking into account the correct version): `sudo apt install libpolly-14-dev`
10. Once the installation is finished running the help command will confirm the installation has completed successfully
    ```
    siderophile --help
    siderophile 0.2.1
    
    USAGE:
        siderophile [FLAGS] [OPTIONS]
    
    FLAGS:
            --include-tests       Count unsafe usage in tests
            --no-mark-closures    Do not mark closures
        -h, --help                Prints help information
        -V, --version             Prints version information
    
    OPTIONS:
            --crate-name <NAME>      Crate name (deprecated)
        -p, --package <SPEC>         Package to be used as the root of the tree
            --mark <TEXT>            Mark bad functions with TEXT
            --threshold <BADNESS>    Minimum badness required to mark a function [default: 0]
        ```



## Installing by building from github
1. Before cloning the siderophile repository we will need to install the git command using: `sudo apt install git`
2. Check the installation by issuing the command : `git --version` it should return a value similar to below:
```
git version 2.43.0
```
3. Create a new directory and change the working directory for the project eg. `mkdir siderophile_build && cd ./siderophile_build`
4. Clone the repo and cd into the siderophile driectory : `git clone https://github.com/trailofbits/siderophile && cd ./siderophile`
5. Use the cargo build command to build the project : `cargo build`
6. You may get the following error while compiling openssl-sys : `error: failed to run custom build command for openssl-sys v0.9.99`
7. You can fix the above error by installing the libssl-dev package : `sudo apt install pkg-config libssl-dev`
8. Reload the profile : `. ~/.proofile`
9. You may get an error as below:
```
error: No suitable version of LLVM was found system-wide or pointed
              to by LLVM_SYS_170_PREFIX.
```
10. There are two possible fixes, and you may need to action one or both
	- The first way to fix this is to install the missing version, the version is the number in the error text above eg. 140 or perhaps 170. In the above instance it will be: `sudo apt install llvm-17`
	- Or you may need to set the library in the cargo build command by issuing the command: `LLVM_SYS_170_PREFIX=/usr/lib/llvm-17 cargo build`
11. You may get an error about a missing library `Polly` as below
```
error: could not find native static library `Polly`, perhaps an -L flag is missing?
```
12. To fix this error you can install the missing package(once again taking into account the correct version): `sudo apt install libpolly-17-dev`
13. Reload the profile : `. ~/.profile`
14 You may get an error about a missing linking library `cc` and the description lower down in the error text about `libzstd` missing as below
```
error: linking with `cc` failed: exit status: 1
....
....
/usr/bin/ld: cannot find -lzstd: No such file or directory
```
15. To fix this error you can install the missing package: `sudo apt install libzstd-dev`
16. The compiler may show errors but it will compile
17. Siderophile can then be installed using the command: `cargo install --path .`
18. Running the help command will confirm the installation has completed successfully
    ```
    siderophile --help
    siderophile 0.2.1
    
    USAGE:
        siderophile [FLAGS] [OPTIONS]
    
    FLAGS:
            --include-tests       Count unsafe usage in tests
            --no-mark-closures    Do not mark closures
        -h, --help                Prints help information
        -V, --version             Prints version information
    
    OPTIONS:
            --crate-name <NAME>      Crate name (deprecated)
        -p, --package <SPEC>         Package to be used as the root of the tree
            --mark <TEXT>            Mark bad functions with TEXT
            --threshold <BADNESS>    Minimum badness required to mark a function [default: 0]
    ```

