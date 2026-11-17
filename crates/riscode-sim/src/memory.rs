//! Memoria con el mismo mapa que RARS:
//!
//! | Región            | Dirección     |
//! |-------------------|---------------|
//! | .text             | 0x00400000    |
//! | extern            | 0x10000000    |
//! | gp (pantalla)     | 0x10008000    |
//! | .data             | 0x10010000    |
//! | heap (sbrk)       | 0x10040000    |
//! | pila (sp)         | 0x7FFFEFFC ↓  |
//! | MMIO (teclado)    | 0xFFFF0000    |

pub const TEXT_BASE: u32 = 0x0040_0000;
pub const TEXT_SIZE: u32 = 0x0040_0000; // 4 MiB
pub const DATA_REGION_BASE: u32 = 0x1000_0000;
pub const DATA_REGION_SIZE: u32 = 0x0040_0000; // 4 MiB: extern, gp, .data y heap
pub const GP_INIT: u32 = 0x1000_8000;
pub const HEAP_BASE: u32 = 0x1004_0000;
pub const STACK_TOP: u32 = 0x8000_0000;
pub const STACK_SIZE: u32 = 0x0040_0000; // 4 MiB
pub const SP_INIT: u32 = 0x7FFF_EFFC;
pub const MMIO_BASE: u32 = 0xFFFF_0000;

/// Direcciones del simulador de teclado y pantalla de texto de RARS.
pub const KBD_CONTROL: u32 = 0xFFFF_0000;
pub const KBD_DATA: u32 = 0xFFFF_0004;
pub const TX_CONTROL: u32 = 0xFFFF_0008;
pub const TX_DATA: u32 = 0xFFFF_000C;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemError {
    Desalineada { addr: u32, size: u32 },
    FueraDeRango { addr: u32 },
    EscrituraEnTexto { addr: u32 },
}

impl std::fmt::Display for MemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemError::Desalineada { addr, size } => {
                write!(f, "dirección {addr:#010x} no está alineada a {size} bytes")
            }
            MemError::FueraDeRango { addr } => write!(f, "la dirección {addr:#010x} está fuera de la memoria"),
            MemError::EscrituraEnTexto { addr } => {
                write!(f, "intento de escribir en .text ({addr:#010x}); el código no se puede modificar")
            }
        }
    }
}

pub struct Memory {
    text: Vec<u8>,
    data: Vec<u8>,
    stack: Vec<u8>,
    /// Teclado: hay una tecla lista (bit 0 de 0xFFFF0000).
    pub kbd_ready: bool,
    pub kbd_data: u32,
    /// Lo que el programa escribió en 0xFFFF000C (pantalla de texto MMIO).
    pub tx_out: Vec<u8>,
}

impl Memory {
    pub fn new() -> Self {
        Memory {
            text: vec![0; TEXT_SIZE as usize],
            data: vec![0; DATA_REGION_SIZE as usize],
            stack: vec![0; STACK_SIZE as usize],
            kbd_ready: false,
            kbd_data: 0,
            tx_out: Vec::new(),
        }
    }

    fn region(&self, addr: u32, len: u32) -> Option<(&[u8], usize)> {
        let in_r = |base: u32, size: u32| addr >= base && (addr - base) as u64 + len as u64 <= size as u64;
        if in_r(DATA_REGION_BASE, DATA_REGION_SIZE) {
            Some((&self.data, (addr - DATA_REGION_BASE) as usize))
        } else if in_r(STACK_TOP - STACK_SIZE, STACK_SIZE) {
            Some((&self.stack, (addr - (STACK_TOP - STACK_SIZE)) as usize))
        } else if in_r(TEXT_BASE, TEXT_SIZE) {
            Some((&self.text, (addr - TEXT_BASE) as usize))
        } else {
            None
        }
    }

    fn region_mut(&mut self, addr: u32, len: u32) -> Result<(&mut [u8], usize), MemError> {
        let in_r = |base: u32, size: u32| addr >= base && (addr - base) as u64 + len as u64 <= size as u64;
        if in_r(DATA_REGION_BASE, DATA_REGION_SIZE) {
            Ok((&mut self.data, (addr - DATA_REGION_BASE) as usize))
        } else if in_r(STACK_TOP - STACK_SIZE, STACK_SIZE) {
            Ok((&mut self.stack, (addr - (STACK_TOP - STACK_SIZE)) as usize))
        } else if in_r(TEXT_BASE, TEXT_SIZE) {
            Err(MemError::EscrituraEnTexto { addr })
        } else {
            Err(MemError::FueraDeRango { addr })
        }
    }

    /// Un pedazo de memoria para leer directamente (lo usa la pantalla bitmap).
    pub fn slice(&self, addr: u32, len: u32) -> Option<&[u8]> {
        self.region(addr, len).map(|(r, off)| &r[off..off + len as usize])
    }

    /// Carga código o datos al iniciar (sí puede escribir en .text).
    pub fn load(&mut self, addr: u32, bytes: &[u8]) {
        let len = bytes.len() as u32;
        let in_r = |base: u32, size: u32| addr >= base && (addr - base) as u64 + len as u64 <= size as u64;
        let (r, off) = if in_r(TEXT_BASE, TEXT_SIZE) {
            (&mut self.text, (addr - TEXT_BASE) as usize)
        } else if in_r(DATA_REGION_BASE, DATA_REGION_SIZE) {
            (&mut self.data, (addr - DATA_REGION_BASE) as usize)
        } else {
            return;
        };
        r[off..off + bytes.len()].copy_from_slice(bytes);
    }

    fn mmio_read(&mut self, addr: u32) -> Result<u32, MemError> {
        Ok(match addr & !3 {
            KBD_CONTROL => self.kbd_ready as u32,
            KBD_DATA => {
                // Igual que RARS: leer el dato de la tecla apaga el bit de "listo".
                self.kbd_ready = false;
                self.kbd_data
            }
            TX_CONTROL => 1,
            TX_DATA => 0,
            _ if addr >= MMIO_BASE => 0,
            _ => return Err(MemError::FueraDeRango { addr }),
        })
    }

    fn mmio_write(&mut self, addr: u32, val: u32) -> Result<(), MemError> {
        match addr & !3 {
            KBD_CONTROL => self.kbd_ready = val & 1 != 0,
            KBD_DATA => self.kbd_data = val,
            TX_CONTROL => {}
            TX_DATA => self.tx_out.push(val as u8),
            _ if addr >= MMIO_BASE => {}
            _ => return Err(MemError::FueraDeRango { addr }),
        }
        Ok(())
    }

    pub fn read(&mut self, addr: u32, size: u32) -> Result<u32, MemError> {
        if addr % size != 0 {
            return Err(MemError::Desalineada { addr, size });
        }
        if addr >= MMIO_BASE {
            let w = self.mmio_read(addr)?;
            let shift = (addr & 3) * 8;
            return Ok(match size {
                4 => w,
                2 => (w >> shift) & 0xFFFF,
                _ => (w >> shift) & 0xFF,
            });
        }
        let (r, off) = self.region(addr, size).ok_or(MemError::FueraDeRango { addr })?;
        Ok(match size {
            4 => u32::from_le_bytes([r[off], r[off + 1], r[off + 2], r[off + 3]]),
            2 => u16::from_le_bytes([r[off], r[off + 1]]) as u32,
            _ => r[off] as u32,
        })
    }

    /// Lee sin efectos secundarios (para los paneles de la interfaz).
    pub fn peek_u32(&self, addr: u32) -> Option<u32> {
        if addr >= MMIO_BASE {
            return Some(match addr & !3 {
                KBD_CONTROL => self.kbd_ready as u32,
                KBD_DATA => self.kbd_data,
                TX_CONTROL => 1,
                _ => 0,
            });
        }
        let (r, off) = self.region(addr, 4)?;
        Some(u32::from_le_bytes([r[off], r[off + 1], r[off + 2], r[off + 3]]))
    }

    pub fn write(&mut self, addr: u32, size: u32, val: u32) -> Result<(), MemError> {
        if addr % size != 0 {
            return Err(MemError::Desalineada { addr, size });
        }
        if addr >= MMIO_BASE {
            if size == 4 {
                return self.mmio_write(addr, val);
            }
            let shift = (addr & 3) * 8;
            let mask = if size == 2 { 0xFFFF } else { 0xFF };
            let old = self.peek_u32(addr & !3).unwrap_or(0);
            let new = (old & !(mask << shift)) | ((val & mask) << shift);
            return self.mmio_write(addr & !3, new);
        }
        let (r, off) = self.region_mut(addr, size)?;
        let b = val.to_le_bytes();
        r[off..off + size as usize].copy_from_slice(&b[..size as usize]);
        Ok(())
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
