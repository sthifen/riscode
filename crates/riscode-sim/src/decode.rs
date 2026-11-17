//! Decodificador: de palabra de 32 bits a instrucción (el camino inverso del ensamblador).

use riscode_asm::lexer::ABI_NAMES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alu {
    Add,
    Sub,
    Sll,
    Slt,
    Sltu,
    Xor,
    Srl,
    Sra,
    Or,
    And,
    Mul,
    Mulh,
    Mulhsu,
    Mulhu,
    Div,
    Divu,
    Rem,
    Remu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Br {
    Eq,
    Ne,
    Lt,
    Ge,
    Ltu,
    Geu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ld {
    B,
    H,
    W,
    Bu,
    Hu,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CsrOp {
    Rw,
    Rs,
    Rc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instr {
    Lui { rd: u8, imm: u32 },
    Auipc { rd: u8, imm: u32 },
    Jal { rd: u8, off: i32 },
    Jalr { rd: u8, rs1: u8, off: i32 },
    Branch { f: Br, rs1: u8, rs2: u8, off: i32 },
    Load { f: Ld, rd: u8, rs1: u8, off: i32 },
    Store { size: u8, rs1: u8, rs2: u8, off: i32 },
    AluImm { f: Alu, rd: u8, rs1: u8, imm: i32 },
    Alu { f: Alu, rd: u8, rs1: u8, rs2: u8 },
    Csr { f: CsrOp, rd: u8, src: u8, imm: bool, csr: u16 },
    Ecall,
    Ebreak,
    Fence,
    Illegal(u32),
}

fn bits(w: u32, hi: u32, lo: u32) -> u32 {
    (w >> lo) & ((1 << (hi - lo + 1)) - 1)
}

fn sext(v: u32, nbits: u32) -> i32 {
    let s = 32 - nbits;
    ((v << s) as i32) >> s
}

pub fn decode(w: u32) -> Instr {
    let op = w & 0x7F;
    let rd = bits(w, 11, 7) as u8;
    let f3 = bits(w, 14, 12);
    let rs1 = bits(w, 19, 15) as u8;
    let rs2 = bits(w, 24, 20) as u8;
    let f7 = bits(w, 31, 25);
    let imm_i = sext(w >> 20, 12);
    match op {
        0x37 => Instr::Lui { rd, imm: w & 0xFFFF_F000 },
        0x17 => Instr::Auipc { rd, imm: w & 0xFFFF_F000 },
        0x6F => {
            let imm = (bits(w, 31, 31) << 20) | (bits(w, 19, 12) << 12) | (bits(w, 20, 20) << 11) | (bits(w, 30, 21) << 1);
            Instr::Jal { rd, off: sext(imm, 21) }
        }
        0x67 if f3 == 0 => Instr::Jalr { rd, rs1, off: imm_i },
        0x63 => {
            let imm = (bits(w, 31, 31) << 12) | (bits(w, 7, 7) << 11) | (bits(w, 30, 25) << 5) | (bits(w, 11, 8) << 1);
            let f = match f3 {
                0 => Br::Eq,
                1 => Br::Ne,
                4 => Br::Lt,
                5 => Br::Ge,
                6 => Br::Ltu,
                7 => Br::Geu,
                _ => return Instr::Illegal(w),
            };
            Instr::Branch { f, rs1, rs2, off: sext(imm, 13) }
        }
        0x03 => {
            let f = match f3 {
                0 => Ld::B,
                1 => Ld::H,
                2 => Ld::W,
                4 => Ld::Bu,
                5 => Ld::Hu,
                _ => return Instr::Illegal(w),
            };
            Instr::Load { f, rd, rs1, off: imm_i }
        }
        0x23 => {
            let size = match f3 {
                0 => 1,
                1 => 2,
                2 => 4,
                _ => return Instr::Illegal(w),
            };
            let imm = (bits(w, 31, 25) << 5) | bits(w, 11, 7);
            Instr::Store { size, rs1, rs2, off: sext(imm, 12) }
        }
        0x13 => {
            let f = match f3 {
                0 => Alu::Add,
                2 => Alu::Slt,
                3 => Alu::Sltu,
                4 => Alu::Xor,
                6 => Alu::Or,
                7 => Alu::And,
                1 if f7 == 0 => return Instr::AluImm { f: Alu::Sll, rd, rs1, imm: rs2 as i32 },
                5 if f7 == 0 => return Instr::AluImm { f: Alu::Srl, rd, rs1, imm: rs2 as i32 },
                5 if f7 == 0x20 => return Instr::AluImm { f: Alu::Sra, rd, rs1, imm: rs2 as i32 },
                _ => return Instr::Illegal(w),
            };
            Instr::AluImm { f, rd, rs1, imm: imm_i }
        }
        0x33 => {
            let f = match (f7, f3) {
                (0, 0) => Alu::Add,
                (0x20, 0) => Alu::Sub,
                (0, 1) => Alu::Sll,
                (0, 2) => Alu::Slt,
                (0, 3) => Alu::Sltu,
                (0, 4) => Alu::Xor,
                (0, 5) => Alu::Srl,
                (0x20, 5) => Alu::Sra,
                (0, 6) => Alu::Or,
                (0, 7) => Alu::And,
                (1, 0) => Alu::Mul,
                (1, 1) => Alu::Mulh,
                (1, 2) => Alu::Mulhsu,
                (1, 3) => Alu::Mulhu,
                (1, 4) => Alu::Div,
                (1, 5) => Alu::Divu,
                (1, 6) => Alu::Rem,
                (1, 7) => Alu::Remu,
                _ => return Instr::Illegal(w),
            };
            Instr::Alu { f, rd, rs1, rs2 }
        }
        0x0F => Instr::Fence,
        0x73 => match f3 {
            0 if w == 0x0000_0073 => Instr::Ecall,
            0 if w == 0x0010_0073 => Instr::Ebreak,
            1..=3 | 5..=7 => {
                let f = match f3 & 3 {
                    1 => CsrOp::Rw,
                    2 => CsrOp::Rs,
                    _ => CsrOp::Rc,
                };
                Instr::Csr { f, rd, src: rs1, imm: f3 >= 5, csr: (w >> 20) as u16 }
            }
            _ => Instr::Illegal(w),
        },
        _ => Instr::Illegal(w),
    }
}

fn r(n: u8) -> &'static str {
    ABI_NAMES[n as usize]
}

fn alu_name(f: Alu) -> &'static str {
    match f {
        Alu::Add => "add",
        Alu::Sub => "sub",
        Alu::Sll => "sll",
        Alu::Slt => "slt",
        Alu::Sltu => "sltu",
        Alu::Xor => "xor",
        Alu::Srl => "srl",
        Alu::Sra => "sra",
        Alu::Or => "or",
        Alu::And => "and",
        Alu::Mul => "mul",
        Alu::Mulh => "mulh",
        Alu::Mulhsu => "mulhsu",
        Alu::Mulhu => "mulhu",
        Alu::Div => "div",
        Alu::Divu => "divu",
        Alu::Rem => "rem",
        Alu::Remu => "remu",
    }
}

/// Texto de la instrucción "básica" (como la columna Basic de RARS). `pc` sirve para mostrar
/// la dirección de destino de los saltos.
pub fn disasm(w: u32, pc: u32) -> String {
    match decode(w) {
        Instr::Lui { rd, imm } => format!("lui {}, {:#x}", r(rd), imm >> 12),
        Instr::Auipc { rd, imm } => format!("auipc {}, {:#x}", r(rd), imm >> 12),
        Instr::Jal { rd, off } => format!("jal {}, {:#010x}", r(rd), pc.wrapping_add(off as u32)),
        Instr::Jalr { rd, rs1, off } => format!("jalr {}, {}({})", r(rd), off, r(rs1)),
        Instr::Branch { f, rs1, rs2, off } => {
            let n = match f {
                Br::Eq => "beq",
                Br::Ne => "bne",
                Br::Lt => "blt",
                Br::Ge => "bge",
                Br::Ltu => "bltu",
                Br::Geu => "bgeu",
            };
            format!("{n} {}, {}, {:#010x}", r(rs1), r(rs2), pc.wrapping_add(off as u32))
        }
        Instr::Load { f, rd, rs1, off } => {
            let n = match f {
                Ld::B => "lb",
                Ld::H => "lh",
                Ld::W => "lw",
                Ld::Bu => "lbu",
                Ld::Hu => "lhu",
            };
            format!("{n} {}, {}({})", r(rd), off, r(rs1))
        }
        Instr::Store { size, rs1, rs2, off } => {
            let n = match size {
                1 => "sb",
                2 => "sh",
                _ => "sw",
            };
            format!("{n} {}, {}({})", r(rs2), off, r(rs1))
        }
        Instr::AluImm { f, rd, rs1, imm } => format!("{}i {}, {}, {}", alu_name(f), r(rd), r(rs1), imm),
        Instr::Alu { f, rd, rs1, rs2 } => format!("{} {}, {}, {}", alu_name(f), r(rd), r(rs1), r(rs2)),
        Instr::Csr { f, rd, src, imm, csr } => {
            let n = match f {
                CsrOp::Rw => "csrrw",
                CsrOp::Rs => "csrrs",
                CsrOp::Rc => "csrrc",
            };
            if imm {
                format!("{n}i {}, {:#x}, {}", r(rd), csr, src)
            } else {
                format!("{n} {}, {:#x}, {}", r(rd), csr, r(src))
            }
        }
        Instr::Ecall => "ecall".into(),
        Instr::Ebreak => "ebreak".into(),
        Instr::Fence => "fence".into(),
        Instr::Illegal(w) => format!("(inválida {w:#010x})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        // Todo lo que el ensamblador genera se debe poder decodificar.
        let src = include_str!("../../riscode-asm/tests/golden/todas_las_instrucciones.asm");
        let p = riscode_asm::assemble(src).unwrap();
        for (i, w) in p.text.iter().enumerate() {
            assert!(!matches!(decode(*w), Instr::Illegal(_)), "palabra {i}: {w:#010x}");
        }
        assert_eq!(disasm(0x00000293, 0x400000), "addi t0, zero, 0");
        assert_eq!(disasm(0x00112023, 0), "sw ra, 0(sp)");
    }
}
