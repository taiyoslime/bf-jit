use std::{error, io};

mod bytecode;
mod token;
mod vm;

pub fn run<R: io::Read, W: io::Write>(
    codes: &str,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn error::Error>> {
    let tokens = token::tokenize(codes)?;
    let bytecodes = bytecode::compile(&tokens)?;
    let program = vm::Program { bytecodes };
    let mut vm = vm::VM::new();
    let _ = vm.run(&program, reader, writer)?;
    Ok(())
}

pub fn run_with_jit<R: io::Read, W: io::Write>(
    codes: &str,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), Box<dyn error::Error>> {
    unimplemented!();
}
