use crate::bytecode::Inst;
use crate::vm::EOF;
use crate::vm::MEMSIZE;
use std::arch::asm;
use std::{error, fmt, ptr};

use std::collections::BTreeMap;

use libc::c_void;

fn codegen(bytecodes: &[Inst]) -> Result<Vec<u8>, CogenError> {
    if !(cfg!(target_os = "linux") || cfg!(target_os = "macos")) {
        return Err(CogenError::UnsupportedOS);
    }
    if !cfg!(target_arch = "x86_64") {
        return Err(CogenError::UnsupportedArch);
    }

    // TODO: mmapする領域に直書き
    let mut machine_codes = vec![];

    let mut stack_loop = vec![]; // TODO: loop用の構造をparse時点で作る
    let mut jmp_abort = vec![];

    // r12: mem + mem_ptr
    // r13: MEMSIZE - 1
    // r14: mem
    // r15: abort

    //stack alignment(tmp)
    machine_codes.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);

    for inst in bytecodes.iter() {
        match inst {
            Inst::MOVPTR(_v) => {
                let v = *_v % MEMSIZE as isize;

                // TODO: gen macro
                if -128 <= v && v <= 127 {
                    // addq r12, #{v}
                    machine_codes.extend_from_slice(&[0x49, 0x83, 0xC4, v as u8])
                } else {
                    // movabs rax, #{v}
                    machine_codes.extend_from_slice(&[0x48, 0xB8]);
                    machine_codes.extend_from_slice(&(v.to_le_bytes()));
                    // add r12, rax
                    machine_codes.extend_from_slice(&[0x49, 0x01, 0xC4]);
                }

                // underflow / overflow
                // mov rax, r12
                // sub rax, r14
                // cmp rax, r13 ; 0 > mem_ptr/*rax*/ || MEMSIZE - 1/*r13*/ < mem_ptr/*rax*/
                // jbe .abort_mem
                machine_codes.extend_from_slice(&[
                    0x4C, 0x89, 0xE0, 0x4C, 0x29, 0xF0, 0x4C, 0x39, 0xE8, 0x0F, 0x87, 0xAF, 0xBE,
                    0xAD, 0xDE,
                ]);
                jmp_abort.push(machine_codes.len());
            }
            Inst::ADD(v) => {
                // addb [r12], #{v}
                machine_codes.extend_from_slice(&[0x41, 0x80, 0x04, 0x24, *v as u8])
            }
            Inst::SETZERO => {
                // movb [r12], 0
                machine_codes.extend_from_slice(&[0x41, 0xC6, 0x04, 0x24, 0x00])
            }
            Inst::MULINTO(coef, _offset) => {
                let offset = *_offset % MEMSIZE as isize;
                // MEMO: cell sizeはu8なので，-255 <= coef <= 255

                // ; r11 <= mem_ptr_to
                // mov r11, r12
                machine_codes.extend_from_slice(&[0x4D, 0x89, 0xE3]);

                if -128 <= offset && offset <= 127 {
                    // addq r11, #{offset}
                    machine_codes.extend_from_slice(&[0x49, 0x83, 0xC3, offset as u8])
                } else {
                    // movabs rax, #{offset}
                    // add r11, rax
                    machine_codes.extend_from_slice(&[0x48, 0xB8]);
                    machine_codes.extend_from_slice(&(offset.to_le_bytes()));
                    machine_codes.extend_from_slice(&[0x49, 0x01, 0xC3]);
                }

                // mov rax, r11
                // sub rax, r14
                // cmp rax, r13
                // jbe .abort_mem
                machine_codes.extend_from_slice(&[
                    0x4C, 0x89, 0xD8, 0x4C, 0x29, 0xF0, 0x4C, 0x39, 0xE8, 0x0F, 0x87, 0xAF, 0xBE,
                    0xAD, 0xDE,
                ]);
                jmp_abort.push(machine_codes.len());

                // movzxb eax, [r12]
                // imul eax, eax, #{coef}
                // addb [r11], al
                // movb [r12], 0
                machine_codes.extend_from_slice(&[0x41, 0x0F, 0xB6, 0x04, 0x24, 0x69, 0xC0]);
                machine_codes.extend_from_slice(&(*coef as i32).to_le_bytes());
                machine_codes.extend_from_slice(&[0x41, 0x00, 0x03, 0x41, 0xC6, 0x04, 0x24, 0x00]);
            }
            Inst::FINDZERO(_v) => {
                let v = *_v % MEMSIZE as isize;
                if -128 <= v && v <= 127 {
                    // s0:
                    // cmpb [r12], 0x0
                    // je s1
                    // addq r12, #{v}
                    // mov rax, r12
                    // sub rax, r14
                    // cmp rax, r13
                    // jbe .abort_mem
                    // jmp s0
                    // s1:
                    machine_codes.extend_from_slice(&[
                        0x41, 0x80, 0x3C, 0x24, 0x00, 0x74, 0x15, 0x49, 0x83, 0xC4, v as u8, 0x4C,
                        0x89, 0xE0, 0x4C, 0x29, 0xF0, 0x4C, 0x39, 0xE8, 0x0F, 0x87, 0xAF, 0xBE,
                        0xAD, 0xDE,
                    ]);
                    jmp_abort.push(machine_codes.len());
                    machine_codes.extend_from_slice(&[0xEB, 0xE4]);
                } else {
                    // s0:
                    // cmpb [r12], 0x0
                    // je s1
                    // movabs rax, #{v}
                    // add r12, rax
                    // mov rax, r12
                    // sub rax, r14
                    // cmp rax, r13
                    // jbe .abort_mem
                    // jmp s0
                    // s1:
                    machine_codes
                        .extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00, 0x74, 0x1E, 0x48, 0xB8]);
                    machine_codes.extend_from_slice(&(v.to_le_bytes()));
                    machine_codes.extend_from_slice(&[
                        0x49, 0x01, 0xC4, 0x4C, 0x89, 0xE0, 0x4C, 0x29, 0xF0, 0x4C, 0x39, 0xE8,
                        0x0F, 0x87, 0xAF, 0xBE, 0xAD, 0xDE,
                    ]);
                    jmp_abort.push(machine_codes.len());
                    machine_codes.extend_from_slice(&[0xEB, 0xDB]);
                }
            }
            Inst::PUTC => {
                // push rdi
                // push rcx
                // mov rsi, 1
                // mov rdx, r12
                // call rcx
                // pop rcx
                // pop rdi

                machine_codes.extend_from_slice(&[
                    0x57, 0x51, 0x48, 0xC7, 0xC6, 0x01, 0x00, 0x00, 0x00, 0x4C, 0x89, 0xE2, 0xFF,
                    0xD1, 0x59, 0x5F,
                ]);
            }
            Inst::GETC => {
                // push rdi
                // push rcx
                // mov rsi, 0
                // mov rdx, r12
                // call rcx
                // movb [r12], al
                // pop rcx
                // pop rdi

                machine_codes.extend_from_slice(&[
                    0x57, 0x51, 0x48, 0xC7, 0xC6, 0x00, 0x00, 0x00, 0x00, 0x4C, 0x89, 0xE2, 0xFF,
                    0xD1, 0x41, 0x88, 0x04, 0x24, 0x59, 0x5F,
                ]);
            }
            Inst::JZ(_) => {
                // cmpb [r12], 0x0
                machine_codes.extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00]);

                // TODO: loop用の構造をbytecode生成時点で作る
                stack_loop.push(machine_codes.len());

                // je #{placeholder}
                machine_codes.extend_from_slice(&[0x0F, 0x84, 0xAF, 0xBE, 0xAD, 0xDE]);
            }
            Inst::JNZ(_) => {
                // cmpb [r12], 0x0
                machine_codes.extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00]);
                let loop_start_offset = (stack_loop.pop().unwrap() + 6) as u32;
                let loop_end_offset = (machine_codes.len() + 6) as u32;

                // jne #{loop_start}
                machine_codes.extend_from_slice(&[0x0F, 0x85]);

                machine_codes.extend_from_slice(
                    &(loop_start_offset as i32 - loop_end_offset as i32).to_le_bytes(),
                );

                for (i, &bt) in (loop_end_offset - loop_start_offset)
                    .to_le_bytes()
                    .iter()
                    .enumerate()
                {
                    machine_codes[loop_start_offset as usize - 4 + i] = bt;
                }
            }
        }
    }

    // add rsp, 0x8
    // ret
    machine_codes.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08, 0xc3]);

    let j_to = machine_codes.len();
    for &j_from in jmp_abort.iter() {
        for (i, &bt) in (j_to as u32 - j_from as u32)
            .to_le_bytes()
            .iter()
            .enumerate()
        {
            machine_codes[j_from - 4 + i] = bt;
        }
    }

    // .abort_mem:
    // xor rdi, rdi
    // call r15
    machine_codes.extend_from_slice(&[0x48, 0x31, 0xFF, 0x41, 0xFF, 0xD7]);

    if cfg!(debug_assertions) {
        let dump = || -> Result<(), std::io::Error> {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open("dump")?;
            f.write_all(&machine_codes[..])?;
            f.flush()?;
            Ok(())
        };
        if let Err(e) = dump() {
            eprintln!("Error(debug): {e}");
        }
    }

    Ok(machine_codes)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CogenError {
    UnsupportedArch,
    UnsupportedOS,
}

impl fmt::Display for CogenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CogenError::*;
        match self {
            UnsupportedArch => write!(f, "target arch is not supported"),
            UnsupportedOS => write!(f, "target os is not supported"),
        }
    }
}

impl error::Error for CogenError {}

pub struct MachineCodePage {
    mem: *mut c_void,
    size: usize,
}

impl MachineCodePage {
    pub unsafe fn new(machine_codes: &[u8]) -> Self {
        // TODO: 機械語のcopyが無駄なのでmmapした領域に直接書き混むnew_from_bytecodeをデフォルトにしたい
        let size = machine_codes.len();
        let mem = libc::mmap(
            std::ptr::null_mut(),
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
            0,
            0,
        );

        // MEMO: copy ~ memmove, copy_nonoverlapping~ memcpy (https://doc.rust-lang.org/std/ptr/fn.copy_nonoverlapping.html)
        ptr::copy_nonoverlapping(machine_codes.as_ptr(), mem as *mut u8, size);
        libc::mprotect(mem, size, libc::PROT_READ);

        Self { mem, size }
    }

    pub unsafe fn new_from_bytecode(bytecodes: &[Inst]) -> Self {
        unimplemented!();
    }

    pub unsafe fn pre_exec(&self) {
        libc::mprotect(self.mem, self.size, libc::PROT_EXEC);
    }

    pub unsafe fn post_exec(&self) {
        libc::mprotect(self.mem, self.size, libc::PROT_READ);
    }

    fn merge(page: MachineCodePage) {
        unimplemented!();
    }
}

extern "C" fn jit_abort(error_code: u8) {
    match error_code {
        0 => eprintln!("Error: memory out of range"),
        _ => (),
    }
    std::process::exit(1);
}

extern "C" fn jit_io(io: &mut IO, c: u8, buf: &mut u8) -> u8 {
    if c == 0 {
        return io.read();
    } else if c == 1 {
        io.write(buf);
    }
    return 0;
}

pub struct JIT {
    pages: BTreeMap<usize, (usize, MachineCodePage)>,
}

impl JIT {
    pub fn new() -> Self {
        let pages = BTreeMap::new();
        Self { pages }
    }

    pub unsafe fn enter(
        &self,
        bytecodes: &[Inst],
        start: usize,
        end: usize,
        mem: &[u8],
        mem_ptr: usize,
    ) -> usize {
        let pages = self.gen_page(bytecodes, start, end); // TODO

        for page in pages.iter() {
            page.pre_exec();
        }

        let mut next_mem_ptr;

        let mem_start = mem.as_ptr() as usize;
        let mem_cur = mem_start + mem_ptr;
        let page_top_addr = pages[0].mem as usize;

        let abort_addr = jit_abort as usize;

        let mut io = IO {
            writer: &mut std::io::stdout(),
            reader: &mut std::io::stdin(),
        };
        let io_ptr = &mut io as *mut IO;
        let jit_io_addr = jit_io as usize;

        asm!(
            "call {0}",
            "sub r12, r14",
            "mov rax, r12",
            in(reg) page_top_addr,
            out("rax") next_mem_ptr,
            in("rdi") io_ptr,
            in("rcx")  jit_io_addr,
            out("r11") _,
            inout("r12") mem_cur => _,
            inout("r13") MEMSIZE - 1 => _,
            inout("r14") mem_start => _,
            inout("r15") abort_addr => _,
            clobber_abi("C"), // TODO
        );

        for page in pages.iter() {
            page.post_exec();
        }

        next_mem_ptr
    }

    unsafe fn gen_page(
        &self,
        bytecodes: &[Inst],
        start: usize,
        end: usize,
    ) -> Vec<MachineCodePage> {
        // TODO: 機械語のvec生成とcopyが無駄なのでmmapした領域に直接書き込みたい
        // TODO: 既にページが存在するならよしなにやる
        let machine_codes = codegen(&bytecodes[start..end + 1]).unwrap(); // TODO

        let page = MachineCodePage::new(&machine_codes);
        vec![page]
    }

    fn read_fn(&self) {}

    fn write_fn(&self) {}
}

/*
struct TestIO<'a, R: 'a + std::io::Read, W: 'a + std::io::Write> {
    writer: &'a mut W,
    reader: &'a mut R,
}
*/
// TODO
struct IO<'a> {
    writer: &'a mut std::io::Stdout,
    reader: &'a mut std::io::Stdin,
}

impl IO<'_> {
    fn read(&mut self) -> u8 {
        use std::io::Read;
        let mut buf: u8 = 0;
        if self
            .reader
            .read_exact(std::slice::from_mut(&mut buf))
            .is_err()
        {
            buf = EOF;
        }
        buf
    }
    fn write(&mut self, buf: &mut u8) {
        use std::io::Write;
        _ = self.writer.write(std::slice::from_mut(buf));
    }
}
