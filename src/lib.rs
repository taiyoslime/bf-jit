use std::{error, io};

mod bytecode;
mod jit;
mod token;
mod vm;

pub fn run<R: io::Read, W: io::Write>(
    codes: &str,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn error::Error>> {
    _run(codes, reader, writer, false)
}

pub fn run_with_jit<R: io::Read, W: io::Write>(
    codes: &str,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn error::Error>> {
    _run(codes, reader, writer, true)
}

fn _run<R: io::Read, W: io::Write>(
    codes: &str,
    reader: &mut R,
    writer: &mut W,
    jit: bool,
) -> Result<(), Box<dyn error::Error>> {
    let tokens = token::tokenize(codes)?;
    let bytecodes = bytecode::compile(&tokens)?;
    let program = vm::Program { bytecodes };
    let mut vm = vm::VM::new();
    let _ = vm.run(&program, reader, writer, jit)?;
    Ok(())
}
