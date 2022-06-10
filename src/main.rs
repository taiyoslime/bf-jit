use clap::Parser;
use std::{error, fs, io, process};

#[derive(Debug, Parser)]
#[clap(author, about, version)]
struct Args {
    #[clap(short, long)]
    with_jit: bool,

    filename: String,
}

fn _main() -> Result<(), Box<dyn error::Error + 'static>> {
    let args = Args::parse();
    let input = fs::read_to_string(args.filename)?;

    if args.with_jit {
        bf_jit::run_with_jit(&input, &mut io::stdin(), &mut io::stdout())?;
    } else {
        bf_jit::run(&input, &mut io::stdin(), &mut io::stdout())?;
    }
    Ok(())
}

fn main() -> process::ExitCode {
    match _main() {
        Ok(_) => process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            process::ExitCode::FAILURE
        }
    }
}
