use crate::bytecode::Inst;
use crate::jit;
use std::error;
use std::fmt;
use std::io;

pub const MEMSIZE: usize = 100000;
pub const JIT_EXEC_TH: u8 = 5;
const EOF: u8 = 0;

pub struct Program {
    pub bytecodes: Vec<Inst>,
}

pub struct VM {
    mem: [u8; MEMSIZE],
    mem_ptr: usize,
    pc: usize,
}

impl Default for VM {
    fn default() -> Self {
        Self {
            mem: [0; MEMSIZE],
            mem_ptr: MEMSIZE / 2,
            pc: 0,
        }
    }
}

impl VM {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run<R: io::Read, W: io::Write>(
        &mut self,
        program: &Program,
        reader: &mut R,
        writer: &mut W,
        enable_jit: bool,
    ) -> Result<(), RuntimeError> {
        let mut buf: [u8; 1] = [0]; // TODO
        let jit = jit::JIT::new();

        while self.pc < program.bytecodes.len() {
            if enable_jit && self.check_exec_count() > JIT_EXEC_TH {
                let start = self.pc;
                // TODO: ループ単位でやる
                let end = program.bytecodes.len() - 1;
                let next_mem_ptr: usize;
                unsafe {
                    next_mem_ptr =
                        jit.enter(&program.bytecodes, start, end, &self.mem, self.mem_ptr);
                }
                self.mem_ptr = next_mem_ptr;
                self.pc = end + 1;
                continue;
            }
            match program.bytecodes[self.pc] {
                Inst::MOVPTR(v) => {
                    self.mem_ptr = check_memory_bound(self.mem_ptr as isize + v, MEMSIZE)?;
                }
                Inst::ADD(v) => {
                    self.mem[self.mem_ptr] = self.mem[self.mem_ptr].wrapping_add(v as u8);
                }
                Inst::SETZERO => {
                    self.mem[self.mem_ptr] = 0;
                }
                Inst::MULINTO(coef, offset) => {
                    let mem_ptr_to = check_memory_bound(self.mem_ptr as isize + offset, MEMSIZE)?;
                    self.mem[mem_ptr_to] = self.mem[mem_ptr_to]
                        .wrapping_add((coef * self.mem[self.mem_ptr] as isize) as u8);
                    self.mem[self.mem_ptr] = 0;
                }
                Inst::FINDZERO(offset) => {
                    while self.mem[self.mem_ptr] != 0 {
                        self.mem_ptr = check_memory_bound(self.mem_ptr as isize + offset, MEMSIZE)?;
                    }
                }
                Inst::PUTC => {
                    let _ = writer.write(&self.mem[self.mem_ptr..(self.mem_ptr + 1)]);
                }
                Inst::GETC => {
                    if reader.read_exact(&mut buf).is_err() {
                        buf[0] = EOF;
                    }
                    self.mem[self.mem_ptr] = buf[0];
                }
                Inst::JZ(addr) => {
                    if self.mem[self.mem_ptr] == 0 {
                        self.pc = addr;
                        continue;
                    }
                }
                Inst::JNZ(addr) => {
                    if self.mem[self.mem_ptr] != 0 {
                        self.pc = addr;
                        continue;
                    }
                }
            }
            self.pc += 1;
        }
        Ok(())
    }

    fn check_exec_count(&mut self) -> u8 {
        // TODO
        255
    }
}

#[inline(always)]
fn check_memory_bound(v: isize, ceil: usize) -> Result<usize, RuntimeError> {
    if v < 0 || ceil as isize <= v {
        return Err(RuntimeError::MemoryOutofRange);
    }
    Ok(v as usize)
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    MemoryOutofRange,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::RuntimeError::*;
        match self {
            MemoryOutofRange => write!(f, "memory out of range"),
        }
    }
}

impl error::Error for RuntimeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::Inst::*;

    #[test]
    fn run_hello_world() {
        // "+[-->-[>>+>-----<<]<--<---]>-.>>>+.>>..+++[.>]<<<<.+++.------.<<-.>>>>+."
        // include memory underflow
        let bytecodes = vec![
            ADD(1),
            JZ(17),
            ADD(-2),
            MOVPTR(1),
            ADD(-1),
            JZ(12),
            MOVPTR(2),
            ADD(1),
            MOVPTR(1),
            ADD(-5),
            MOVPTR(-2),
            JNZ(6),
            MOVPTR(-1),
            ADD(-2),
            MOVPTR(-1),
            ADD(-3),
            JNZ(2),
            MOVPTR(1),
            ADD(-1),
            PUTC,
            MOVPTR(3),
            ADD(1),
            PUTC,
            MOVPTR(2),
            PUTC,
            PUTC,
            ADD(3),
            JZ(31),
            PUTC,
            MOVPTR(1),
            JNZ(28),
            MOVPTR(-4),
            PUTC,
            ADD(3),
            PUTC,
            ADD(-6),
            PUTC,
            MOVPTR(-2),
            ADD(-1),
            PUTC,
            MOVPTR(4),
            ADD(1),
            PUTC,
        ];
        let mut vm = VM::new();
        let mut output = vec![];
        let _ = vm
            .run(
                &Program { bytecodes },
                &mut "".as_bytes(),
                &mut output,
                false,
            )
            .unwrap();
        assert_eq!(
            "Hello, World!"
                .chars()
                .map(|c| c as u8)
                .collect::<Vec<u8>>(),
            output
        );
    }

    #[test]
    fn run_movev() {
        // "++++++++++>>[-]<<[->>+<<]"
        let bytecodes = vec![ADD(10), MOVPTR(2), SETZERO, MOVPTR(-2), MULINTO(1, 2)];
        let mut vm = VM {
            mem_ptr: 0,
            ..Default::default()
        };
        let _ = vm
            .run(
                &Program { bytecodes },
                &mut "".as_bytes(),
                &mut vec![],
                false,
            )
            .unwrap();
        assert_eq!(vm.mem[0..3], [0, 0, 10]);
    }

    #[test]
    fn run_cat() {
        // ",[.,]"
        // EOF == 0
        let bytecodes = vec![GETC, JZ(5), PUTC, GETC, JNZ(2)];
        let mut vm = VM::new();

        let text = "testtesttesttest\n";

        let mut input = text.as_bytes();
        let mut output = vec![];
        let _ = vm
            .run(&Program { bytecodes }, &mut input, &mut output, false)
            .unwrap();
        assert_eq!(text.chars().map(|c| c as u8).collect::<Vec<u8>>(), output);
    }

    #[test]
    fn run_out_of_range() {
        let bytecodes = vec![
            MOVPTR(MEMSIZE as isize),
        ];
        let mut vm = VM::new();
        let res = vm
            .run(
                &Program { bytecodes },
                &mut "".as_bytes(),
                &mut vec![],
                false,
            );

        assert_eq!(Some(RuntimeError::MemoryOutofRange), res.err());
    }

    #[test]
    fn run_findzero() {
        let bytecodes = vec![
            ADD(1),
            MOVPTR(1),
            ADD(2),
            MOVPTR(1),
            ADD(3),
            MOVPTR(-2),
            FINDZERO(1),
        ];
        let mut vm = VM {
            mem_ptr: 0,
            ..Default::default()
        };
        let _ = vm
            .run(
                &Program { bytecodes },
                &mut "".as_bytes(),
                &mut vec![],
                false,
            )
            .unwrap();
        assert_eq!(vm.mem[0..4], [1, 2, 3, 0]);
        assert_eq!(vm.mem_ptr, 3)
    }
}
