use std::process::ExitCode;

fn main() -> ExitCode {
    pangopup_build::runtime_v2_qualification::main(std::env::args_os().skip(1))
}
