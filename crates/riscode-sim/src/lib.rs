//! # riscode-sim
//!
//! Simulador RISC-V (RV32IM) con el mismo mapa de memoria, syscalls, pantalla bitmap y
//! teclado MMIO que RARS, para correr los programas sin abrir RARS.

pub mod decode;
pub mod java_random;
pub mod machine;
pub mod memory;

pub use decode::{decode, disasm, Instr};
pub use machine::{InputKind, Machine, Stop};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn correr(src: &str) -> Machine {
        let p = riscode_asm::assemble(src).unwrap();
        let mut m = Machine::new(&p);
        let (_, stop) = m.run(10_000_000, &HashSet::new());
        assert!(matches!(stop, Stop::Exit(_)), "{stop:?}");
        m
    }

    #[test]
    fn factorial() {
        let src = "main:\n li a0, 5\n jal fact\n li a7, 1\n ecall\n li a7, 10\n ecall\n\
                   fact:\n li t0, 1\n mv t1, a0\n li a0, 1\n\
                   loop:\n blt t1, t0, fin\n mul a0, a0, t1\n addi t1, t1, -1\n j loop\n\
                   fin:\n ret";
        let m = correr(src);
        assert_eq!(m.output, "120");
    }

    #[test]
    fn memoria_y_texto() {
        let src = ".data\nmsg: .string \"Hola RISCode\"\nnum: .word -7\n.text\n\
                   la a0, msg\n li a7, 4\n ecall\n lw a0, num\n li a7, 1\n ecall\n\
                   li t0, 0x41\n sb t0, 0(sp)\n lbu a0, 0(sp)\n li a7, 11\n ecall";
        let m = correr(src);
        assert_eq!(m.output, "Hola RISCode-7A");
    }

    #[test]
    fn division_y_signos() {
        let src = "li t0, -7\n li t1, 2\n div a0, t0, t1\n rem a1, t0, t1\n divu a2, t0, zero\n\
                   srai a3, t0, 1\n srli a4, t0, 28\n mulh a5, t0, t0";
        let m = correr(src);
        assert_eq!(m.regs[10] as i32, -3);
        assert_eq!(m.regs[11] as i32, -1);
        assert_eq!(m.regs[12], u32::MAX);
        assert_eq!(m.regs[13] as i32, -4);
        assert_eq!(m.regs[14], 0xF);
        assert_eq!(m.regs[15], 0);
    }

    #[test]
    fn teclado_mmio() {
        // espera una tecla y la guarda en a0
        let src = "espera:\n lw t0, 0xFFFF0000\n beqz t0, espera\n lw a0, 0xFFFF0004";
        let p = riscode_asm::assemble(src).unwrap();
        let mut m = Machine::new(&p);
        let (_, stop) = m.run(1000, &HashSet::new());
        assert_eq!(stop, Stop::Budget);
        m.key_press(b'w');
        let (_, stop) = m.run(1000, &HashSet::new());
        assert_eq!(stop, Stop::Exit(0));
        assert_eq!(m.regs[10], b'w' as u32);
        assert!(!m.mem.kbd_ready, "leer el dato apaga el bit de listo");
    }

    #[test]
    fn pantalla_en_gp() {
        let src = "li t0, 0x00FF0000\n sw t0, 0(gp)\n sw t0, 4(gp)";
        let m = correr(src);
        let px = m.mem.slice(memory::GP_INIT, 8).unwrap();
        assert_eq!(px, &[0, 0, 0xFF, 0, 0, 0, 0xFF, 0]);
    }

    #[test]
    fn errores_de_ejecucion() {
        let p = riscode_asm::assemble("li t0, 1\n lw a0, 0(t0)").unwrap();
        let mut m = Machine::new(&p);
        let (_, stop) = m.run(10, &HashSet::new());
        let Stop::Error(msg) = stop else { panic!("{stop:?}") };
        assert!(msg.contains("alineada"), "{msg}");
    }

    #[test]
    fn puntos_de_parada() {
        let p = riscode_asm::assemble("li t0, 1\nli t1, 2\nli t2, 3").unwrap();
        let mut m = Machine::new(&p);
        let bp: HashSet<u32> = [p.line_addr[2].unwrap()].into();
        let (n, stop) = m.run(100, &bp);
        assert_eq!((n, stop), (2, Stop::Breakpoint));
        let (_, stop) = m.run(100, &bp);
        assert_eq!(stop, Stop::Exit(0));
        assert_eq!(m.regs[7], 3);
    }
}
