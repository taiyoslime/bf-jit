use crate::bytecode::Inst;
use crate::vm::MEMSIZE;
use std::arch::asm;
use std::{error, fmt, mem, ptr};

use std::collections::BTreeMap;

use libc::c_void;

// TODO: fn codegen<F: Fn()>(bytecodes: &[Inst], read_fn: F, write_fn: F) -> Result<Vec<u8>, CogenError> {
fn codegen(bytecodes: &[Inst]) -> Result<Vec<u8>, CogenError> {
    // TODO: mmapする領域に直書き
    let mut machine_codes = vec![];

    let mut stack = vec![]; // TODO: loop用の構造をparse時点で作る

    // read_fn/write_fnを呼び出す際のレジスタ退避/復元の手間を省くため，以下のようにレジスタを固定する
    // TODO: r1*で機械語が(rdi, ... に比べて)大きくなることによる影響と，read|write_fn呼び出し時の諸々による影響の比較

    // r12: mem + mem_ptr
    // r13: MEMSIZE
    // r14: mem
    // r15: mem + MEMSIZE (for optim)

    // ret-val(rax): mem_ptr
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
                // TODO: macro
                // xor rax, rax
                // cmp r12, r15
                // cmovg rax, r13
                // neg rax
                // cmp r12, r14
                // cmovl rax, r13
                // add r12, rax
                machine_codes.extend_from_slice(&[
                    0x48, 0x31, 0xC0, 0x4D, 0x39, 0xFC, 0x49, 0x0F, 0x4F, 0xC5, 0x48, 0xF7, 0xD8,
                    0x4D, 0x39, 0xF4, 0x49, 0x0F, 0x4C, 0xC5, 0x49, 0x01, 0xC4,
                ]);
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

                // xor rax, rax
                // cmp r11, r15
                // cmovg rax, r13
                // neg rax
                // cmp r11, r14
                // cmovl rax, r13
                // add r11, rax
                // movzxb eax, [r12]
                // imul eax, eax, #{coef}
                // addb [r11], al
                // movb [r12], 0
                machine_codes.extend_from_slice(&[
                    0x48, 0x31, 0xC0, 0x4D, 0x39, 0xFB, 0x49, 0x0F, 0x4F, 0xC5, 0x48, 0xF7, 0xD8,
                    0x4D, 0x39, 0xF3, 0x49, 0x0F, 0x4C, 0xC5, 0x49, 0x01, 0xC3, 0x41, 0x0F, 0xB6,
                    0x04, 0x24, 0x69, 0xC0,
                ]);
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
                    // xor rax, rax
                    // cmp r12, r15
                    // cmovg rax, r13
                    // neg rax
                    // cmp r12, r14
                    // cmovl rax, r13
                    // add r12, rax
                    // jmp s0
                    // s1:
                    machine_codes.extend_from_slice(&[
                        0x41, 0x80, 0x3C, 0x24, 0x00, 0x74, 0x1D, 0x49, 0x83, 0xC4, v as u8, 0x48,
                        0x31, 0xC0, 0x4D, 0x39, 0xFC, 0x49, 0x0F, 0x4F, 0xC5, 0x48, 0xF7, 0xD8,
                        0x4D, 0x39, 0xF4, 0x49, 0x0F, 0x4C, 0xC5, 0x49, 0x01, 0xC4, 0xEB, 0xDC,
                    ]);
                } else {
                    // ...
                    // movabs rax, #{v}
                    // add r12, rax
                    // ...
                    machine_codes
                        .extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00, 0x74, 0x24, 0x48, 0xB8]);
                    machine_codes.extend_from_slice(&(v.to_le_bytes()));
                    machine_codes.extend_from_slice(&[
                        0x49, 0x01, 0xC4, 0x48, 0x31, 0xC0, 0x4D, 0x39, 0xFC, 0x49, 0x0F, 0x4F,
                        0xC5, 0x48, 0xF7, 0xD8, 0x4D, 0x39, 0xF4, 0x49, 0x0F, 0x4C, 0xC5, 0x49,
                        0x01, 0xC4, 0xEB, 0xD3,
                    ]);
                }
            }
            Inst::PUTC => {
                // TODO: とりあえず動かすためwrite直呼び

                // mov rax, 0x1
                // mov rdi, 0x1
                // mov rsi, r12
                // mov rdx, 0x1
                // syscall

                machine_codes.extend_from_slice(&[
                    0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00, 0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00,
                    0x00, 0x4C, 0x89, 0xE6, 0x48, 0xC7, 0xC2, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x05,
                ]);
            }
            Inst::GETC => {
                // TODO: とりあえず動かすためread直呼び

                // xor rax, rax
                // xor rdi, rdi
                // mov rsi, r12
                // mov rdx, 0x1
                // syscall

                machine_codes.extend_from_slice(&[
                    0x48, 0x31, 0xC0, 0x48, 0x31, 0xFF, 0x4C, 0x89, 0xE6, 0x48, 0xC7, 0xC2, 0x01,
                    0x00, 0x00, 0x00, 0x0F, 0x05,
                ]);
            }
            Inst::JZ(_) => {
                // cmpb [r12], 0x0
                machine_codes.extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00]);

                // TODO: loop用の構造をbytecode生成時点で作る
                stack.push(machine_codes.len());

                // je #{placeholder}
                machine_codes.extend_from_slice(&[0x0F, 0x84, 0xde, 0xad, 0xbe, 0xaf]);
            }
            Inst::JNZ(_) => {
                // cmpb [r12], 0x0
                machine_codes.extend_from_slice(&[0x41, 0x80, 0x3C, 0x24, 0x00]);
                let loop_start_offset = (stack.pop().unwrap() + 6) as u32;
                let loop_end_offset = (machine_codes.len() + 6) as u32;

                // jne #{loop_start}
                machine_codes.extend_from_slice(&[0x0F, 0x85]);

                machine_codes.extend_from_slice(
                    &(loop_start_offset as i32 - loop_end_offset as i32).to_le_bytes(),
                );

                for (i, bt) in (loop_end_offset - loop_start_offset)
                    .to_le_bytes()
                    .iter()
                    .enumerate()
                {
                    machine_codes[loop_start_offset as usize - 4 + i] = *bt;
                }
            }
        }
    }

    // ret
    machine_codes.push(0xc3);

    Ok(machine_codes)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CogenError {}

impl fmt::Display for CogenError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TODO")
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
        let mem_end = mem_start + MEMSIZE;
        let mem_cur = mem_start + mem_ptr;
        let page_top_addr = pages[0].mem as usize;

        asm!(
            "call {0}",
            "sub r12, r14",
            "mov rax, r12",
            in(reg) page_top_addr,
            out("rax") next_mem_ptr,
            out("r11") _,
            inout("r12") mem_cur => _,
            inout("r13") MEMSIZE => _,
            inout("r14") mem_start => _,
            inout("r15") mem_end => _,
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
