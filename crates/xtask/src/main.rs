use std::process::ExitCode;

mod annotations;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("annotations") => annotations::run(args),
        Some(cmd) => {
            eprintln!("xtask: unknown subcommand '{cmd}'");
            ExitCode::from(2)
        }
        None => {
            eprintln!("usage: cargo xtask <annotations>");
            ExitCode::from(2)
        }
    }
}
