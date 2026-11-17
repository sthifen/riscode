//! # riscode-asm
//!
//! Ensamblador RISC-V (RV32IM) con el dialecto de RARS. Es la versión en Rust del ensamblador
//! del Proyecto 3 (Diseño de Sistemas Digitales, TEC) de Steven e Isabel Sevilla.

pub mod assembler;
pub mod highlight;
pub mod instr;
pub mod lexer;

pub use assembler::{assemble, AsmError, Program, DATA_BASE, TEXT_BASE};

/// Formato de salida, igual que los botones "BIN OUTPUT" / "HEX OUTPUT" del Proyecto 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    Bin,
    Hex,
}

/// Una palabra de 32 bits por línea, en binario o en hexadecimal (como el Dump de RARS).
pub fn exportar_palabras(words: impl IntoIterator<Item = u32>, fmt: Formato) -> String {
    let mut s = String::new();
    for w in words {
        match fmt {
            Formato::Bin => s.push_str(&format!("{w:032b}\n")),
            Formato::Hex => s.push_str(&format!("{w:08x}\n")),
        }
    }
    s
}

/// Los bytes de `.data` agrupados en palabras (little endian), completando con ceros.
pub fn palabras_de_datos(data: &[u8]) -> Vec<u32> {
    data.chunks(4)
        .map(|c| {
            let mut b = [0u8; 4];
            b[..c.len()].copy_from_slice(c);
            u32::from_le_bytes(b)
        })
        .collect()
}
