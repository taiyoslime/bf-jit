use crate::bytecode::Inst;
use std::error;
use std::fmt;
use std::io;

const MEMSIZE: usize = 100000;
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
            mem_ptr: 0,
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
    ) -> Result<(), RuntimeError> {
        let mut buf: [u8; 1] = [0]; // TODO
        while self.pc < program.bytecodes.len() {
            match program.bytecodes[self.pc] {
                Inst::MOVPTR(v) => {
                    // https://esolangs.org/wiki/Brainfuck#Memory
                    self.mem_ptr = wrap(self.mem_ptr as isize + v, MEMSIZE);
                }
                Inst::ADD(v) => {
                    self.mem[self.mem_ptr] = self.mem[self.mem_ptr].wrapping_add(v as u8);
                }
                Inst::SETZERO => {
                    self.mem[self.mem_ptr] = 0;
                }
                Inst::MULINTO(coef, offset) => {
                    let mem_ptr_to = wrap(self.mem_ptr as isize + offset, MEMSIZE);
                    self.mem[mem_ptr_to] = self.mem[mem_ptr_to]
                        .wrapping_add((coef * self.mem[self.mem_ptr] as isize) as u8);
                    self.mem[self.mem_ptr] = 0;
                }
                Inst::FINDZERO(offset) => {
                    while self.mem[self.mem_ptr] != 0 {
                        self.mem_ptr = wrap(self.mem_ptr as isize + offset, MEMSIZE);
                    }
                }
                Inst::PUTC => {
                    let _ = writer.write(&self.mem[self.mem_ptr..(self.mem_ptr + 1)]);
                }
                Inst::GETC => {
                    if let Err(_) = reader.read_exact(&mut buf) {
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
}

#[inline(always)]
fn wrap(v: isize, ceil: usize) -> usize {
    if v < 0 {
        return (v % ceil as isize + ceil as isize) as usize;
    }
    if v >= ceil as isize {
        return (v % ceil as isize) as usize;
    }
    v as usize
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TODO")
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
            .run(&Program { bytecodes }, &mut "".as_bytes(), &mut output)
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
        let mut vm = VM::new();
        let _ = vm
            .run(&Program { bytecodes }, &mut "".as_bytes(), &mut vec![])
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
            .run(&Program { bytecodes }, &mut input, &mut output)
            .unwrap();
        assert_eq!(text.chars().map(|c| c as u8).collect::<Vec<u8>>(), output);
    }

    #[test]
    fn run_overflow_and_underflow() {
        let bytecodes = vec![
            ADD(isize::MAX),
            MOVPTR(MEMSIZE as isize * 128 + 1),
            ADD(isize::MIN),
            MOVPTR(MEMSIZE as isize * -24 - 2),
            ADD(23098120392),
        ];
        let mut vm = VM::new();
        let _ = vm
            .run(&Program { bytecodes }, &mut "".as_bytes(), &mut vec![])
            .unwrap();

        assert_eq!([vm.mem[0], vm.mem[1], vm.mem[MEMSIZE - 1]], [255, 0, 200]);
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
        let mut vm = VM::new();
        let _ = vm
            .run(&Program { bytecodes }, &mut "".as_bytes(), &mut vec![])
            .unwrap();
        assert_eq!(vm.mem[0..4], [1, 2, 3, 0]);
        assert_eq!(vm.mem_ptr, 3)
    }
}
