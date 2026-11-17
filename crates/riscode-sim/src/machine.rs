//! La CPU: registros, pc, ciclo de ejecución y llamadas al sistema (ecall) de RARS.

use std::collections::{HashMap, HashSet};

use riscode_asm::Program;

use crate::decode::{decode, Alu, Br, CsrOp, Instr, Ld};
use crate::java_random::JavaRandom;
use crate::memory::{Memory, GP_INIT, HEAP_BASE, SP_INIT, TEXT_BASE};

/// Por qué se detuvo la ejecución.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Se acabó el presupuesto de instrucciones de esta tanda: se puede seguir.
    Budget,
    /// El programa pidió dormir (syscall 32) esta cantidad de milisegundos.
    Sleep(u32),
    /// El programa terminó (syscall 10/17/93 o se acabó el código).
    Exit(i32),
    /// El programa espera que el usuario escriba algo en la consola (syscalls 5, 8, 12).
    NeedInput(InputKind),
    /// Llegó a un punto de parada.
    Breakpoint,
    /// Error en tiempo de ejecución.
    Error(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    Int,
    String,
    Char,
}

pub struct Machine {
    pub regs: [u32; 32],
    pub pc: u32,
    pub mem: Memory,
    code: Vec<Instr>,
    text_end: u32,
    pub instr_count: u64,
    /// Texto que el programa imprimió (la interfaz lo va vaciando).
    pub output: String,
    rngs: HashMap<u32, JavaRandom>,
    heap: u32,
    csrs: HashMap<u16, u32>,
    pending_input: Option<String>,
    pub exited: Option<i32>,
    /// Registro que cambió en la última instrucción (para resaltarlo en la interfaz).
    pub last_written: Option<u8>,
    /// Hora actual en ms desde 1970 (la pone quien ejecuta, para la syscall 30).
    pub now_ms: u64,
}

impl Machine {
    pub fn new(prog: &Program) -> Self {
        let mut mem = Memory::new();
        let text_bytes: Vec<u8> = prog.text.iter().flat_map(|w| w.to_le_bytes()).collect();
        mem.load(TEXT_BASE, &text_bytes);
        mem.load(riscode_asm::DATA_BASE, &prog.data);
        let mut regs = [0u32; 32];
        regs[2] = SP_INIT;
        regs[3] = GP_INIT;
        Machine {
            regs,
            pc: TEXT_BASE,
            mem,
            code: prog.text.iter().map(|w| decode(*w)).collect(),
            text_end: prog.text_end(),
            instr_count: 0,
            output: String::new(),
            rngs: HashMap::new(),
            heap: HEAP_BASE,
            csrs: HashMap::new(),
            pending_input: None,
            exited: None,
            last_written: None,
            now_ms: 0,
        }
    }

    /// Una tecla del teclado MMIO (como escribir en el "Keyboard and Display MMIO Simulator").
    pub fn key_press(&mut self, ascii: u8) {
        self.mem.kbd_data = ascii as u32;
        self.mem.kbd_ready = true;
    }

    /// Respuesta del usuario cuando el programa pidió entrada (Stop::NeedInput).
    pub fn provide_input(&mut self, text: String) {
        self.pending_input = Some(text);
    }

    /// Ejecuta hasta `max` instrucciones. Se detiene antes en un punto de parada
    /// (excepto si es la primera instrucción, para poder continuar desde uno).
    pub fn run(&mut self, max: u64, breakpoints: &HashSet<u32>) -> (u64, Stop) {
        if let Some(code) = self.exited {
            return (0, Stop::Exit(code));
        }
        let mut n = 0;
        while n < max {
            if n > 0 && breakpoints.contains(&self.pc) {
                return (n, Stop::Breakpoint);
            }
            match self.step_inner() {
                None => n += 1,
                Some(Stop::NeedInput(k)) => return (n, Stop::NeedInput(k)),
                Some(stop) => return (n + 1, stop),
            }
        }
        (n, Stop::Budget)
    }

    /// Ejecuta una sola instrucción.
    pub fn step(&mut self) -> Stop {
        if let Some(code) = self.exited {
            return Stop::Exit(code);
        }
        self.step_inner().unwrap_or(Stop::Budget)
    }

    #[inline]
    fn set(&mut self, rd: u8, v: u32) {
        if rd != 0 {
            self.regs[rd as usize] = v;
            self.last_written = Some(rd);
        }
    }

    fn fail(&mut self, msg: String) -> Option<Stop> {
        Some(Stop::Error(format!("{msg} (pc = {:#010x})", self.pc)))
    }

    fn step_inner(&mut self) -> Option<Stop> {
        let pc = self.pc;
        if pc >= self.text_end || pc < TEXT_BASE {
            if pc == self.text_end {
                // Igual que RARS: se acabó el código, el programa termina.
                self.exited = Some(0);
                return Some(Stop::Exit(0));
            }
            return self.fail(format!("el pc salió del código: {pc:#010x}"));
        }
        if pc % 4 != 0 {
            return self.fail("el pc no está alineado a 4 bytes".into());
        }
        let ins = self.code[((pc - TEXT_BASE) / 4) as usize];
        self.last_written = None;
        let x = |r: u8| self.regs[r as usize];
        let mut next = pc.wrapping_add(4);
        match ins {
            Instr::Lui { rd, imm } => self.set(rd, imm),
            Instr::Auipc { rd, imm } => self.set(rd, pc.wrapping_add(imm)),
            Instr::Jal { rd, off } => {
                self.set(rd, next);
                next = pc.wrapping_add(off as u32);
            }
            Instr::Jalr { rd, rs1, off } => {
                let target = x(rs1).wrapping_add(off as u32) & !1;
                self.set(rd, next);
                next = target;
            }
            Instr::Branch { f, rs1, rs2, off } => {
                let (a, b) = (x(rs1), x(rs2));
                let take = match f {
                    Br::Eq => a == b,
                    Br::Ne => a != b,
                    Br::Lt => (a as i32) < (b as i32),
                    Br::Ge => (a as i32) >= (b as i32),
                    Br::Ltu => a < b,
                    Br::Geu => a >= b,
                };
                if take {
                    next = pc.wrapping_add(off as u32);
                }
            }
            Instr::Load { f, rd, rs1, off } => {
                let addr = x(rs1).wrapping_add(off as u32);
                let size = match f {
                    Ld::B | Ld::Bu => 1,
                    Ld::H | Ld::Hu => 2,
                    Ld::W => 4,
                };
                let v = match self.mem.read(addr, size) {
                    Ok(v) => v,
                    Err(e) => return self.fail(format!("al leer: {e}")),
                };
                let v = match f {
                    Ld::B => v as u8 as i8 as i32 as u32,
                    Ld::H => v as u16 as i16 as i32 as u32,
                    _ => v,
                };
                self.set(rd, v);
            }
            Instr::Store { size, rs1, rs2, off } => {
                let addr = x(rs1).wrapping_add(off as u32);
                if let Err(e) = self.mem.write(addr, size as u32, x(rs2)) {
                    return self.fail(format!("al escribir: {e}"));
                }
            }
            Instr::AluImm { f, rd, rs1, imm } => {
                let v = alu(f, x(rs1), imm as u32);
                self.set(rd, v);
            }
            Instr::Alu { f, rd, rs1, rs2 } => {
                let v = alu(f, x(rs1), x(rs2));
                self.set(rd, v);
            }
            Instr::Csr { f, rd, src, imm, csr } => {
                let old = self.read_csr(csr);
                let val = if imm { src as u32 } else { x(src) };
                let new = match f {
                    CsrOp::Rw => Some(val),
                    CsrOp::Rs if src != 0 => Some(old | val),
                    CsrOp::Rc if src != 0 => Some(old & !val),
                    _ => None,
                };
                if let Some(v) = new {
                    self.csrs.insert(csr, v);
                }
                self.set(rd, old);
            }
            Instr::Fence => {}
            Instr::Ebreak => {
                self.pc = next;
                self.instr_count += 1;
                return Some(Stop::Breakpoint);
            }
            Instr::Ecall => {
                match self.syscall() {
                    Ok(None) => {}
                    Ok(Some(Stop::NeedInput(k))) => return Some(Stop::NeedInput(k)),
                    Ok(Some(stop)) => {
                        self.pc = next;
                        self.instr_count += 1;
                        return Some(stop);
                    }
                    Err(e) => return self.fail(e),
                }
            }
            Instr::Illegal(w) => return self.fail(format!("instrucción inválida {w:#010x}")),
        }
        self.pc = next;
        self.instr_count += 1;
        None
    }

    fn read_csr(&self, csr: u16) -> u32 {
        match csr {
            0xC00 | 0xC02 => self.instr_count as u32,
            0xC80 | 0xC82 => (self.instr_count >> 32) as u32,
            0xC01 => self.now_ms as u32,
            0xC81 => (self.now_ms >> 32) as u32,
            _ => self.csrs.get(&csr).copied().unwrap_or(0),
        }
    }

    fn read_cstring(&mut self, mut addr: u32) -> Result<String, String> {
        let mut bytes = Vec::new();
        loop {
            let b = self.mem.read(addr, 1).map_err(|e| e.to_string())? as u8;
            if b == 0 || bytes.len() > 1 << 20 {
                break;
            }
            bytes.push(b);
            addr = addr.wrapping_add(1);
        }
        Ok(String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect()))
    }

    fn rng(&mut self, id: u32) -> &mut JavaRandom {
        let seed = self.now_ms as i64;
        self.rngs.entry(id).or_insert_with(|| JavaRandom::new(seed))
    }

    /// Llamadas al sistema de RARS. El número va en a7, los argumentos en a0..a2.
    fn syscall(&mut self) -> Result<Option<Stop>, String> {
        let a0 = self.regs[10];
        let a1 = self.regs[11];
        let a7 = self.regs[17];
        match a7 {
            1 => self.output.push_str(&(a0 as i32).to_string()),
            4 => {
                let s = self.read_cstring(a0)?;
                self.output.push_str(&s);
            }
            5 | 8 | 12 => {
                let kind = match a7 {
                    5 => InputKind::Int,
                    8 => InputKind::String,
                    _ => InputKind::Char,
                };
                let Some(text) = self.pending_input.take() else {
                    return Ok(Some(Stop::NeedInput(kind)));
                };
                match kind {
                    InputKind::Int => {
                        let t = text.trim();
                        let v = riscode_asm::lexer::parse_number(t.trim_start_matches('-'))
                            .map(|v| if t.starts_with('-') { -v } else { v })
                            .ok_or_else(|| format!("'{t}' no es un número entero"))?;
                        self.set(10, v as u32);
                    }
                    InputKind::Char => {
                        let c = text.chars().next().unwrap_or('\n');
                        self.set(10, c as u32);
                    }
                    InputKind::String => {
                        // a0 = buffer, a1 = tamaño máximo (incluye el 0 final)
                        let max = a1 as usize;
                        let mut bytes: Vec<u8> = text.into_bytes();
                        bytes.push(b'\n');
                        bytes.truncate(max.saturating_sub(1));
                        for (i, b) in bytes.iter().chain(std::iter::once(&0u8)).enumerate() {
                            self.mem.write(a0.wrapping_add(i as u32), 1, *b as u32).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
            9 => {
                // sbrk: reserva a0 bytes en el heap
                let addr = self.heap;
                self.heap = self.heap.wrapping_add(a0).next_multiple_of(4);
                self.set(10, addr);
            }
            10 => {
                self.exited = Some(0);
                return Ok(Some(Stop::Exit(0)));
            }
            11 => self.output.push(char::from_u32(a0 & 0xFF).unwrap_or('?')),
            17 | 93 => {
                self.exited = Some(a0 as i32);
                return Ok(Some(Stop::Exit(a0 as i32)));
            }
            30 => {
                self.set(10, self.now_ms as u32);
                self.set(11, (self.now_ms >> 32) as u32);
            }
            31 => {} // MIDI: sin sonido por ahora
            32 => return Ok(Some(Stop::Sleep(a0))),
            33 => return Ok(Some(Stop::Sleep(a1))), // MIDI síncrono: espera la duración
            34 => self.output.push_str(&format!("{a0:#010x}")),
            35 => self.output.push_str(&format!("{a0:032b}")),
            36 => self.output.push_str(&a0.to_string()),
            40 => {
                self.rngs.insert(a0, JavaRandom::new(a1 as i32 as i64));
            }
            41 => {
                let v = self.rng(a0).next_int();
                self.set(10, v as u32);
            }
            42 => {
                if (a1 as i32) <= 0 {
                    return Err("syscall 42: el límite superior (a1) debe ser positivo".into());
                }
                let v = self.rng(a0).next_int_bound(a1 as i32);
                self.set(10, v as u32);
            }
            55 => {
                let s = self.read_cstring(a0)?;
                self.output.push_str(&format!("{s}\n"));
            }
            56 => {
                let s = self.read_cstring(a0)?;
                self.output.push_str(&format!("{s}{}\n", a1 as i32));
            }
            other => return Err(format!("la syscall {other} (a7) no está soportada todavía")),
        }
        Ok(None)
    }
}

fn alu(f: Alu, a: u32, b: u32) -> u32 {
    let (sa, sb) = (a as i32, b as i32);
    match f {
        Alu::Add => a.wrapping_add(b),
        Alu::Sub => a.wrapping_sub(b),
        Alu::Sll => a << (b & 31),
        Alu::Slt => (sa < sb) as u32,
        Alu::Sltu => (a < b) as u32,
        Alu::Xor => a ^ b,
        Alu::Srl => a >> (b & 31),
        Alu::Sra => (sa >> (b & 31)) as u32,
        Alu::Or => a | b,
        Alu::And => a & b,
        Alu::Mul => a.wrapping_mul(b),
        Alu::Mulh => ((sa as i64 * sb as i64) >> 32) as u32,
        Alu::Mulhsu => ((sa as i64).wrapping_mul(b as i64) >> 32) as u32,
        Alu::Mulhu => ((a as u64 * b as u64) >> 32) as u32,
        Alu::Div => {
            if b == 0 {
                u32::MAX
            } else {
                sa.wrapping_div(sb) as u32
            }
        }
        Alu::Divu => {
            if b == 0 {
                u32::MAX
            } else {
                a / b
            }
        }
        Alu::Rem => {
            if b == 0 {
                a
            } else {
                sa.wrapping_rem(sb) as u32
            }
        }
        Alu::Remu => {
            if b == 0 {
                a
            } else {
                a % b
            }
        }
    }
}
