//! Tabla de instrucciones RISC-V.
//!
//! Es la misma tabla `instrucciones_riscv[]` del Proyecto 3 (opcode / funct3 / funct7),
//! pero con los números ya en hexadecimal en vez de cadenas de bits, más una pequeña
//! documentación en español para mostrar al pasar el mouse en el editor.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Formato {
    R,
    I,
    /// I con shamt (slli, srli, srai)
    IShift,
    /// Cargas: lw rd, off(rs1)
    ILoad,
    S,
    SB,
    U,
    UJ,
    /// jalr
    IJalr,
    /// ecall / ebreak / fence
    Sistema,
    Csr,
    CsrI,
}

#[derive(Clone, Copy, Debug)]
pub struct InstruccionRiscv {
    pub nombre: &'static str,
    pub formato: Formato,
    pub opcode: u32,
    pub funct3: u32,
    pub funct7: u32,
}

const fn ins(nombre: &'static str, formato: Formato, opcode: u32, funct3: u32, funct7: u32) -> InstruccionRiscv {
    InstruccionRiscv { nombre, formato, opcode, funct3, funct7 }
}

use Formato::*;

pub const INSTRUCCIONES: &[InstruccionRiscv] = &[
    // Tipo I: carga
    ins("lb", ILoad, 0x03, 0b000, 0),
    ins("lh", ILoad, 0x03, 0b001, 0),
    ins("lw", ILoad, 0x03, 0b010, 0),
    ins("lbu", ILoad, 0x03, 0b100, 0),
    ins("lhu", ILoad, 0x03, 0b101, 0),
    // Tipo I: aritméticas inmediatas
    ins("addi", I, 0x13, 0b000, 0),
    ins("slti", I, 0x13, 0b010, 0),
    ins("sltiu", I, 0x13, 0b011, 0),
    ins("xori", I, 0x13, 0b100, 0),
    ins("ori", I, 0x13, 0b110, 0),
    ins("andi", I, 0x13, 0b111, 0),
    ins("slli", IShift, 0x13, 0b001, 0x00),
    ins("srli", IShift, 0x13, 0b101, 0x00),
    ins("srai", IShift, 0x13, 0b101, 0x20),
    // Tipo R
    ins("add", R, 0x33, 0b000, 0x00),
    ins("sub", R, 0x33, 0b000, 0x20),
    ins("sll", R, 0x33, 0b001, 0x00),
    ins("slt", R, 0x33, 0b010, 0x00),
    ins("sltu", R, 0x33, 0b011, 0x00),
    ins("xor", R, 0x33, 0b100, 0x00),
    ins("srl", R, 0x33, 0b101, 0x00),
    ins("sra", R, 0x33, 0b101, 0x20),
    ins("or", R, 0x33, 0b110, 0x00),
    ins("and", R, 0x33, 0b111, 0x00),
    // Extensión M
    ins("mul", R, 0x33, 0b000, 0x01),
    ins("mulh", R, 0x33, 0b001, 0x01),
    ins("mulhsu", R, 0x33, 0b010, 0x01),
    ins("mulhu", R, 0x33, 0b011, 0x01),
    ins("div", R, 0x33, 0b100, 0x01),
    ins("divu", R, 0x33, 0b101, 0x01),
    ins("rem", R, 0x33, 0b110, 0x01),
    ins("remu", R, 0x33, 0b111, 0x01),
    // Tipo S
    ins("sb", S, 0x23, 0b000, 0),
    ins("sh", S, 0x23, 0b001, 0),
    ins("sw", S, 0x23, 0b010, 0),
    // Tipo SB
    ins("beq", SB, 0x63, 0b000, 0),
    ins("bne", SB, 0x63, 0b001, 0),
    ins("blt", SB, 0x63, 0b100, 0),
    ins("bge", SB, 0x63, 0b101, 0),
    ins("bltu", SB, 0x63, 0b110, 0),
    ins("bgeu", SB, 0x63, 0b111, 0),
    // Tipo U
    ins("lui", U, 0x37, 0, 0),
    ins("auipc", U, 0x17, 0, 0),
    // Tipo UJ
    ins("jal", UJ, 0x6F, 0, 0),
    ins("jalr", IJalr, 0x67, 0b000, 0),
    // Sistema
    ins("ecall", Sistema, 0x73, 0, 0),
    ins("ebreak", Sistema, 0x73, 0, 1),
    ins("fence", Sistema, 0x0F, 0, 0),
    ins("fence.i", Sistema, 0x0F, 1, 0),
    ins("csrrw", Csr, 0x73, 0b001, 0),
    ins("csrrs", Csr, 0x73, 0b010, 0),
    ins("csrrc", Csr, 0x73, 0b011, 0),
    ins("csrrwi", CsrI, 0x73, 0b101, 0),
    ins("csrrsi", CsrI, 0x73, 0b110, 0),
    ins("csrrci", CsrI, 0x73, 0b111, 0),
];

pub fn buscar_instruccion(nombre: &str) -> Option<&'static InstruccionRiscv> {
    INSTRUCCIONES.iter().find(|i| i.nombre == nombre)
}

/// Pseudoinstrucciones que el ensamblador expande (como en RARS).
pub const PSEUDO: &[&str] = &[
    "li", "la", "mv", "not", "neg", "nop", "j", "jr", "ret", "call", "tail", "beqz", "bnez", "blez",
    "bgez", "bltz", "bgtz", "bgt", "ble", "bgtu", "bleu", "seqz", "snez", "sltz", "sgtz", "csrr",
    "csrw", "csrs", "csrc", "csrwi", "csrsi", "csrci", "rdcycle", "rdtime", "rdinstret",
];

pub fn es_mnemonico(n: &str) -> bool {
    buscar_instruccion(n).is_some() || PSEUDO.contains(&n)
}

pub fn es_pseudo(n: &str) -> bool {
    PSEUDO.contains(&n)
}

pub const DIRECTIVAS: &[&str] = &[
    ".text", ".data", ".word", ".half", ".byte", ".space", ".string", ".asciz", ".ascii", ".align",
    ".eqv", ".globl", ".global", ".extern",
];

/// CSRs con nombre (los de modo usuario de RARS y los contadores).
pub fn csr_num(name: &str) -> Option<u16> {
    Some(match name {
        "ustatus" => 0x000,
        "fflags" => 0x001,
        "frm" => 0x002,
        "fcsr" => 0x003,
        "uie" => 0x004,
        "utvec" => 0x005,
        "uscratch" => 0x040,
        "uepc" => 0x041,
        "ucause" => 0x042,
        "utval" => 0x043,
        "uip" => 0x044,
        "cycle" => 0xC00,
        "time" => 0xC01,
        "instret" => 0xC02,
        "cycleh" => 0xC80,
        "timeh" => 0xC81,
        "instreth" => 0xC82,
        _ => return None,
    })
}

/// Documentación corta para el tooltip del editor: (sintaxis, descripción).
pub fn documentacion(n: &str) -> Option<(&'static str, &'static str)> {
    Some(match n {
        "add" => ("add rd, rs1, rs2", "rd = rs1 + rs2"),
        "sub" => ("sub rd, rs1, rs2", "rd = rs1 - rs2"),
        "and" => ("and rd, rs1, rs2", "rd = rs1 & rs2 (AND bit a bit)"),
        "or" => ("or rd, rs1, rs2", "rd = rs1 | rs2 (OR bit a bit)"),
        "xor" => ("xor rd, rs1, rs2", "rd = rs1 ^ rs2 (XOR bit a bit)"),
        "sll" => ("sll rd, rs1, rs2", "rd = rs1 << rs2 (desplazamiento lógico a la izquierda)"),
        "srl" => ("srl rd, rs1, rs2", "rd = rs1 >> rs2 (lógico, rellena con 0)"),
        "sra" => ("sra rd, rs1, rs2", "rd = rs1 >> rs2 (aritmético, conserva el signo)"),
        "slt" => ("slt rd, rs1, rs2", "rd = 1 si rs1 < rs2 (con signo), si no 0"),
        "sltu" => ("sltu rd, rs1, rs2", "rd = 1 si rs1 < rs2 (sin signo), si no 0"),
        "addi" => ("addi rd, rs1, imm", "rd = rs1 + imm (imm de 12 bits: -2048..2047)"),
        "andi" => ("andi rd, rs1, imm", "rd = rs1 & imm"),
        "ori" => ("ori rd, rs1, imm", "rd = rs1 | imm"),
        "xori" => ("xori rd, rs1, imm", "rd = rs1 ^ imm"),
        "slti" => ("slti rd, rs1, imm", "rd = 1 si rs1 < imm (con signo)"),
        "sltiu" => ("sltiu rd, rs1, imm", "rd = 1 si rs1 < imm (sin signo)"),
        "slli" => ("slli rd, rs1, shamt", "rd = rs1 << shamt (0..31)"),
        "srli" => ("srli rd, rs1, shamt", "rd = rs1 >> shamt (lógico)"),
        "srai" => ("srai rd, rs1, shamt", "rd = rs1 >> shamt (aritmético)"),
        "lw" => ("lw rd, off(rs1)", "Carga una palabra (4 bytes) de memoria[rs1 + off]"),
        "lh" => ("lh rd, off(rs1)", "Carga media palabra (2 bytes) con signo"),
        "lhu" => ("lhu rd, off(rs1)", "Carga media palabra (2 bytes) sin signo"),
        "lb" => ("lb rd, off(rs1)", "Carga un byte con signo"),
        "lbu" => ("lbu rd, off(rs1)", "Carga un byte sin signo"),
        "sw" => ("sw rs2, off(rs1)", "Guarda una palabra en memoria[rs1 + off]"),
        "sh" => ("sh rs2, off(rs1)", "Guarda media palabra (2 bytes)"),
        "sb" => ("sb rs2, off(rs1)", "Guarda un byte"),
        "beq" => ("beq rs1, rs2, etiqueta", "Salta si rs1 == rs2"),
        "bne" => ("bne rs1, rs2, etiqueta", "Salta si rs1 != rs2"),
        "blt" => ("blt rs1, rs2, etiqueta", "Salta si rs1 < rs2 (con signo)"),
        "bge" => ("bge rs1, rs2, etiqueta", "Salta si rs1 >= rs2 (con signo)"),
        "bltu" => ("bltu rs1, rs2, etiqueta", "Salta si rs1 < rs2 (sin signo)"),
        "bgeu" => ("bgeu rs1, rs2, etiqueta", "Salta si rs1 >= rs2 (sin signo)"),
        "lui" => ("lui rd, imm20", "rd = imm20 << 12 (carga los 20 bits altos)"),
        "auipc" => ("auipc rd, imm20", "rd = pc + (imm20 << 12)"),
        "jal" => ("jal [rd,] etiqueta", "Salta a la etiqueta y guarda la dirección de regreso en rd (ra por defecto)"),
        "jalr" => ("jalr rd, off(rs1)", "Salta a rs1 + off y guarda la dirección de regreso en rd"),
        "ecall" => ("ecall", "Llamada al sistema: el número va en a7 (1 imprimir entero, 4 texto, 10 salir, 32 pausa, 42 aleatorio…)"),
        "ebreak" => ("ebreak", "Detiene la ejecución (punto de parada)"),
        "mul" => ("mul rd, rs1, rs2", "rd = (rs1 * rs2), 32 bits bajos"),
        "mulh" => ("mulh rd, rs1, rs2", "32 bits altos de rs1 * rs2 (con signo)"),
        "mulhu" => ("mulhu rd, rs1, rs2", "32 bits altos de rs1 * rs2 (sin signo)"),
        "mulhsu" => ("mulhsu rd, rs1, rs2", "32 bits altos de rs1 (con signo) * rs2 (sin signo)"),
        "div" => ("div rd, rs1, rs2", "rd = rs1 / rs2 (con signo)"),
        "divu" => ("divu rd, rs1, rs2", "rd = rs1 / rs2 (sin signo)"),
        "rem" => ("rem rd, rs1, rs2", "rd = rs1 % rs2 (con signo)"),
        "remu" => ("remu rd, rs1, rs2", "rd = rs1 % rs2 (sin signo)"),
        "li" => ("li rd, imm", "Pseudo: carga un número de 32 bits (addi, o lui + addi)"),
        "la" => ("la rd, etiqueta", "Pseudo: carga la dirección de una etiqueta (auipc + addi)"),
        "mv" => ("mv rd, rs", "Pseudo: rd = rs (addi rd, rs, 0)"),
        "not" => ("not rd, rs", "Pseudo: rd = ~rs (xori rd, rs, -1)"),
        "neg" => ("neg rd, rs", "Pseudo: rd = -rs (sub rd, x0, rs)"),
        "nop" => ("nop", "Pseudo: no hace nada (addi x0, x0, 0)"),
        "j" => ("j etiqueta", "Pseudo: salto incondicional (jal x0, etiqueta)"),
        "jr" => ("jr rs", "Pseudo: salta a la dirección en rs (jalr x0, 0(rs))"),
        "ret" => ("ret", "Pseudo: regresa de una función (jalr x0, 0(ra))"),
        "call" => ("call etiqueta", "Pseudo: llama a una función lejana (auipc + jalr)"),
        "tail" => ("tail etiqueta", "Pseudo: salto lejano sin guardar regreso"),
        "beqz" => ("beqz rs, etiqueta", "Pseudo: salta si rs == 0"),
        "bnez" => ("bnez rs, etiqueta", "Pseudo: salta si rs != 0"),
        "blez" => ("blez rs, etiqueta", "Pseudo: salta si rs <= 0"),
        "bgez" => ("bgez rs, etiqueta", "Pseudo: salta si rs >= 0"),
        "bltz" => ("bltz rs, etiqueta", "Pseudo: salta si rs < 0"),
        "bgtz" => ("bgtz rs, etiqueta", "Pseudo: salta si rs > 0"),
        "bgt" => ("bgt rs1, rs2, etiqueta", "Pseudo: salta si rs1 > rs2 (blt rs2, rs1)"),
        "ble" => ("ble rs1, rs2, etiqueta", "Pseudo: salta si rs1 <= rs2 (bge rs2, rs1)"),
        "bgtu" => ("bgtu rs1, rs2, etiqueta", "Pseudo: salta si rs1 > rs2 sin signo"),
        "bleu" => ("bleu rs1, rs2, etiqueta", "Pseudo: salta si rs1 <= rs2 sin signo"),
        "seqz" => ("seqz rd, rs", "Pseudo: rd = 1 si rs == 0"),
        "snez" => ("snez rd, rs", "Pseudo: rd = 1 si rs != 0"),
        "sltz" => ("sltz rd, rs", "Pseudo: rd = 1 si rs < 0"),
        "sgtz" => ("sgtz rd, rs", "Pseudo: rd = 1 si rs > 0"),
        ".text" => (".text", "Empieza la sección de código (0x00400000)"),
        ".data" => (".data", "Empieza la sección de datos (0x10010000)"),
        ".word" => (".word v1, v2, …", "Reserva palabras de 4 bytes con esos valores"),
        ".half" => (".half v1, …", "Reserva medias palabras de 2 bytes"),
        ".byte" => (".byte v1, …", "Reserva bytes"),
        ".space" => (".space n", "Reserva n bytes en cero"),
        ".string" | ".asciz" => (".string \"texto\"", "Texto terminado en 0"),
        ".ascii" => (".ascii \"texto\"", "Texto sin el 0 final"),
        ".align" => (".align n", "Alinea a 2^n bytes"),
        ".eqv" => (".eqv NOMBRE, valor", "Define una constante: NOMBRE se reemplaza por valor"),
        ".globl" | ".global" => (".globl etiqueta", "Marca una etiqueta como global (se ignora)"),
        _ => return None,
    })
}
