use crate::token::Token;
use std::{error, fmt};
mod optimize;

#[derive(PartialEq, Debug)]
pub enum Inst {
    MOVPTR(isize),
    ADD(isize),
    SETZERO,
    MULINTO(isize, isize), // (coef, offset)
    PUTC,
    GETC,
    JZ(usize),
    JNZ(usize),
}

pub fn compile(tokens: &Vec<Token>) -> Result<Vec<Inst>, CompileError> {
    let mut insts = vec![];
    let mut addr: usize = 0;
    let mut acc_val: isize = 1;
    let mut stack = vec![];
    let len_tokens = tokens.len();

    for i in 0..len_tokens {
        match tokens[i] {
            Token::LT => {
                if i == len_tokens - 1 || tokens[i + 1] != Token::LT {
                    insts.push(Inst::MOVPTR(-acc_val));
                    addr += 1;
                    acc_val = 1;
                } else {
                    acc_val += 1;
                }
            }
            Token::GT => {
                if i == len_tokens - 1 || tokens[i + 1] != Token::GT {
                    insts.push(Inst::MOVPTR(acc_val));
                    addr += 1;
                    acc_val = 1;
                } else {
                    acc_val += 1;
                }
            }
            Token::PLUS => {
                if i == len_tokens - 1 || tokens[i + 1] != Token::PLUS {
                    insts.push(Inst::ADD(acc_val));
                    addr += 1;
                    acc_val = 1;
                } else {
                    acc_val += 1;
                }
            }
            Token::MINUS => {
                if i == len_tokens - 1 || tokens[i + 1] != Token::MINUS {
                    insts.push(Inst::ADD(-acc_val));
                    addr += 1;
                    acc_val = 1;
                } else {
                    acc_val += 1;
                }
            }
            Token::DOT => {
                insts.push(Inst::PUTC);
                addr += 1;
            }
            Token::COMMA => {
                insts.push(Inst::GETC);
                addr += 1;
            }
            Token::LSQB => {
                stack.push((addr, i));
                insts.push(Inst::JZ(0)); // temp
                addr += 1;
            }
            Token::RSQB => {
                if let Some((saved_addr, _)) = stack.pop() {
                    // replase "[-]" to SET(0)
                    if addr >= 2 {
                        match insts[addr - 2..addr] {
                            // TODO: other case in v
                            [Inst::JZ(_), Inst::ADD(v)] if v == -1 || v == 1 => {
                                insts.pop();
                                insts.pop();
                                insts.push(Inst::SETZERO);
                                addr -= 1;
                                continue;
                            }
                            _ => (),
                        }
                    }

                    if addr >= 5 {
                        match insts[addr - 5..addr] {
                            [Inst::JZ(_), Inst::ADD(v0), Inst::MOVPTR(p0), Inst::ADD(v1), Inst::MOVPTR(p1)]
                            | [Inst::JZ(_), Inst::MOVPTR(p0), Inst::ADD(v1), Inst::MOVPTR(p1), Inst::ADD(v0)]
                                if p0.abs() == p1.abs() && p0 != p1 && v0 == -1 =>
                            {
                                for _ in 0..5 {
                                    insts.pop();
                                }
                                insts.push(Inst::MULINTO(v1, p0));
                                addr -= 4;
                                continue;
                            }
                            _ => (),
                        }
                    }

                    insts[saved_addr] = Inst::JZ(addr + 1);
                    insts.push(Inst::JNZ(saved_addr + 1));
                    addr += 1;
                } else {
                    return Err(CompileError::RSQBMismatch(i));
                }
            }
        }
    }

    if !stack.is_empty() {
        // TODO
        let (_, pos) = stack[0];
        return Err(CompileError::LSQBMismatch(pos));
    }

    Ok(insts)
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompileError {
    LSQBMismatch(usize),
    RSQBMismatch(usize),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::CompileError::*;
        match self {
            // TODO
            LSQBMismatch(pos) => write!(f, "unclosed '[' at pos {}", pos),
            RSQBMismatch(pos) => write!(f, "unexpected ']' at pos {}", pos),
        }
    }
}

impl error::Error for CompileError {}

#[cfg(test)]
mod tests {
    use super::Inst::*;
    use super::*;
    use crate::token::Token::*;
    #[test]
    fn compile_movev() {
        let tokens = vec![
            GT, GT, LSQB, MINUS, RSQB, LT, LT, LSQB, MINUS, GT, GT, PLUS, LT, LT, RSQB,
        ];
        let insts = compile(&tokens);
        assert_eq!(
            vec![MOVPTR(2), SETZERO, MOVPTR(-2), MULINTO(1, 2)],
            insts.unwrap()
        );
    }

    #[test]
    fn compile_lsqb_error() {
        let tokens = vec![LSQB, PLUS, PLUS];
        let insts = compile(&tokens);
        assert_eq!(Some(CompileError::LSQBMismatch(0)), insts.err());
    }

    #[test]
    fn compile_rsqb_error() {
        let tokens = vec![LSQB, PLUS, PLUS, RSQB, RSQB];
        let insts = compile(&tokens);
        assert_eq!(Some(CompileError::RSQBMismatch(4)), insts.err());
    }
}
