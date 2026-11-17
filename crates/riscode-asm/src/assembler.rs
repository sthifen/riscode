//! Ensamblador de dos pasadas (igual que el Proyecto 3):
//!
//! * `primera_pasada`: lee línea por línea, guarda las constantes `.eqv`, las etiquetas con su
//!   dirección y cuánto ocupa cada instrucción (las pseudoinstrucciones pueden ocupar 2).
//! * `segunda_pasada`: con todas las etiquetas ya conocidas, codifica cada instrucción a
//!   código máquina y llena la sección `.data`.
//!
//! El dialecto es el de RARS, y la salida es la misma que el "Dump" de RARS.

use std::collections::HashMap;

use crate::instr::{self, Formato};
use crate::lexer::{reg_num, strip_comment, tokenize, Tok, Token};

pub const TEXT_BASE: u32 = 0x0040_0000;
pub const DATA_BASE: u32 = 0x1001_0000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    /// Línea (empezando en 0).
    pub line: usize,
    /// Columnas en bytes dentro de la línea.
    pub start: usize,
    pub end: usize,
    pub msg: String,
}

#[derive(Clone, Debug, Default)]
pub struct Program {
    /// Código máquina de `.text`, empezando en `TEXT_BASE`.
    pub text: Vec<u32>,
    /// Línea del fuente (desde 0) de cada palabra de `text`.
    pub text_lines: Vec<usize>,
    /// Bytes de `.data`, empezando en `DATA_BASE`.
    pub data: Vec<u8>,
    /// Etiquetas y su dirección.
    pub labels: HashMap<String, u32>,
    /// Primera dirección de código de cada línea del fuente (si la línea genera código).
    pub line_addr: Vec<Option<u32>>,
}

impl Program {
    pub fn text_end(&self) -> u32 {
        TEXT_BASE + 4 * self.text.len() as u32
    }

    /// Línea del fuente de la instrucción en `pc`.
    pub fn line_of_pc(&self, pc: u32) -> Option<usize> {
        if pc < TEXT_BASE || pc >= self.text_end() {
            return None;
        }
        self.text_lines.get(((pc - TEXT_BASE) / 4) as usize).copied()
    }
}

#[derive(Clone, Debug)]
struct Expr {
    sym: Option<String>,
    val: i64,
}

#[derive(Clone, Debug)]
enum OpKind {
    Reg(u8),
    Imm(Expr),
    Mem(Expr, u8),
}

#[derive(Clone, Debug)]
struct Op {
    kind: OpKind,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
enum DataDir {
    Word,
    Half,
    Byte,
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug)]
struct DataItem {
    expr: Expr,
    count: u32,
    start: usize,
    end: usize,
}

#[derive(Clone, Debug)]
enum Body {
    Instr { mn: String, ops: Vec<Op> },
    Data { dir: DataDir, items: Vec<DataItem> },
}

#[derive(Clone, Debug)]
struct Stmt {
    line: usize,
    start: usize,
    end: usize,
    addr: u32,
    body: Body,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Seg {
    Text,
    Data,
}

struct Asm {
    errors: Vec<AsmError>,
    eqv: HashMap<String, Vec<Tok>>,
    labels: HashMap<String, u32>,
    label_line: HashMap<String, usize>,
    stmts: Vec<Stmt>,
    seg: Seg,
    text_pc: u32,
    data_pc: u32,
    pending_data_labels: Vec<String>,
    n_lines: usize,
}

/// Ensambla un programa completo. Si hay errores, los devuelve todos (no solo el primero).
pub fn assemble(src: &str) -> Result<Program, Vec<AsmError>> {
    let mut a = Asm {
        errors: Vec::new(),
        eqv: HashMap::new(),
        labels: HashMap::new(),
        label_line: HashMap::new(),
        stmts: Vec::new(),
        seg: Seg::Text,
        text_pc: TEXT_BASE,
        data_pc: DATA_BASE,
        pending_data_labels: Vec::new(),
        n_lines: 0,
    };
    a.primera_pasada(src);
    // La segunda pasada se hace aunque haya errores, para reportarlos todos de una vez
    // (por ejemplo etiquetas que no existen), como hace VS Code.
    let prog = a.segunda_pasada();
    if !a.errors.is_empty() {
        a.errors.sort_by_key(|e| (e.line, e.start));
        a.errors.dedup_by_key(|e| (e.line, e.start));
        return Err(a.errors);
    }
    Ok(prog)
}

fn err(line: usize, start: usize, end: usize, msg: impl Into<String>) -> AsmError {
    AsmError { line, start, end, msg: msg.into() }
}

fn fits_i12(v: i64) -> bool {
    (-2048..=2047).contains(&v)
}

/// Divide un valor de 32 bits en (20 bits altos, 12 bits bajos con signo), para lui/auipc + addi.
fn hi_lo(v: i64) -> (u32, i32) {
    let v = v as i32;
    let lo = (v << 20) >> 20;
    let hi = (v.wrapping_sub(lo) as u32) >> 12;
    (hi, lo)
}

// ───────── codificación de formatos ─────────
fn enc_r(f7: u32, rs2: u8, rs1: u8, f3: u32, rd: u8, op: u32) -> u32 {
    (f7 << 25) | ((rs2 as u32) << 20) | ((rs1 as u32) << 15) | (f3 << 12) | ((rd as u32) << 7) | op
}
fn enc_i(imm: i32, rs1: u8, f3: u32, rd: u8, op: u32) -> u32 {
    (((imm as u32) & 0xFFF) << 20) | ((rs1 as u32) << 15) | (f3 << 12) | ((rd as u32) << 7) | op
}
fn enc_s(imm: i32, rs2: u8, rs1: u8, f3: u32, op: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7F) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (f3 << 12)
        | ((imm & 0x1F) << 7)
        | op
}
fn enc_b(imm: i32, rs2: u8, rs1: u8, f3: u32) -> u32 {
    let imm = imm as u32;
    (((imm >> 12) & 1) << 31)
        | (((imm >> 5) & 0x3F) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (f3 << 12)
        | (((imm >> 1) & 0xF) << 8)
        | (((imm >> 11) & 1) << 7)
        | 0x63
}
fn enc_u(imm20: u32, rd: u8, op: u32) -> u32 {
    ((imm20 & 0xFFFFF) << 12) | ((rd as u32) << 7) | op
}
fn enc_j(imm: i32, rd: u8) -> u32 {
    let imm = imm as u32;
    (((imm >> 20) & 1) << 31)
        | (((imm >> 1) & 0x3FF) << 21)
        | (((imm >> 11) & 1) << 20)
        | (((imm >> 12) & 0xFF) << 12)
        | ((rd as u32) << 7)
        | 0x6F
}

const ADDI: (u32, u32) = (0x13, 0b000);
const RA: u8 = 1;

fn branch_f3(mn: &str) -> u32 {
    instr::buscar_instruccion(mn).map(|i| i.funct3).unwrap_or(0)
}

impl Asm {
    fn error(&mut self, line: usize, start: usize, end: usize, msg: impl Into<String>) {
        self.errors.push(err(line, start, end, msg));
    }

    // ═════════════════════ PRIMERA PASADA ═════════════════════
    fn primera_pasada(&mut self, src: &str) {
        for (no, raw) in src.lines().enumerate() {
            self.n_lines = no + 1;
            let code = strip_comment(raw);
            let toks = match tokenize(code) {
                Ok(t) => t,
                Err((s, e, m)) => {
                    self.error(no, s, e, m);
                    continue;
                }
            };
            self.procesar_linea(no, &toks);
        }
        self.bind_pending_data_labels();
    }

    fn procesar_linea(&mut self, no: usize, toks: &[Token]) {
        let mut i = 0;
        // etiquetas: "nombre:" (puede haber varias en la misma línea)
        while i + 1 < toks.len() && toks[i + 1].tok == Tok::Colon {
            if let Tok::Ident(name) = &toks[i].tok {
                self.definir_etiqueta(no, name.clone(), toks[i].start, toks[i].end);
                i += 2;
            } else {
                break;
            }
        }
        if i >= toks.len() {
            return;
        }
        let head_tok = &toks[i];
        let Tok::Ident(head) = &head_tok.tok else {
            self.error(no, head_tok.start, head_tok.end, "se esperaba una instrucción, directiva o etiqueta");
            return;
        };
        let head = head.to_ascii_lowercase();
        let rest = &toks[i + 1..];
        if head == ".eqv" {
            self.directiva_eqv(no, head_tok, rest);
            return;
        }
        let rest = match self.sustituir_eqv(no, rest) {
            Ok(r) => r,
            Err(e) => {
                self.errors.push(e);
                return;
            }
        };
        if head.starts_with('.') {
            self.directiva(no, &head, head_tok, &rest);
        } else {
            self.instruccion(no, &head, head_tok, &rest);
        }
    }

    fn definir_etiqueta(&mut self, no: usize, name: String, start: usize, end: usize) {
        if let Some(prev) = self.label_line.get(&name) {
            let prev = *prev;
            self.error(no, start, end, format!("la etiqueta '{name}' ya está definida en la línea {}", prev + 1));
            return;
        }
        self.label_line.insert(name.clone(), no);
        match self.seg {
            Seg::Text => {
                self.labels.insert(name, self.text_pc);
            }
            // En .data la dirección se decide en la siguiente directiva (por la alineación).
            Seg::Data => self.pending_data_labels.push(name),
        }
    }

    fn bind_pending_data_labels(&mut self) {
        for name in std::mem::take(&mut self.pending_data_labels) {
            self.labels.insert(name, self.data_pc);
        }
    }

    /// Reemplaza los nombres definidos con `.eqv` por su valor (como hace RARS).
    fn sustituir_eqv(&self, no: usize, toks: &[Token]) -> Result<Vec<Token>, AsmError> {
        let mut out = Vec::with_capacity(toks.len());
        for t in toks {
            self.expandir_token(no, t, t.start, t.end, &mut out, 0)?;
        }
        Ok(out)
    }

    fn expandir_token(
        &self,
        no: usize,
        t: &Token,
        start: usize,
        end: usize,
        out: &mut Vec<Token>,
        depth: usize,
    ) -> Result<(), AsmError> {
        if let Tok::Ident(name) = &t.tok {
            if let Some(val) = self.eqv.get(name) {
                if depth > 16 {
                    return Err(err(no, start, end, format!("'.eqv {name}' se define a sí mismo")));
                }
                for v in val {
                    let tk = Token { tok: v.clone(), start, end };
                    self.expandir_token(no, &tk, start, end, out, depth + 1)?;
                }
                return Ok(());
            }
        }
        out.push(Token { tok: t.tok.clone(), start, end });
        Ok(())
    }

    fn directiva_eqv(&mut self, no: usize, head: &Token, rest: &[Token]) {
        let Some(Token { tok: Tok::Ident(name), .. }) = rest.first() else {
            self.error(no, head.start, head.end, "uso: .eqv NOMBRE, valor");
            return;
        };
        let mut j = 1;
        if rest.get(1).map(|t| &t.tok) == Some(&Tok::Comma) {
            j = 2;
        }
        if j >= rest.len() {
            self.error(no, head.start, rest[0].end, format!("a '.eqv {name}' le falta el valor"));
            return;
        }
        match self.sustituir_eqv(no, &rest[j..]) {
            Ok(v) => {
                self.eqv.insert(name.clone(), v.into_iter().map(|t| t.tok).collect());
            }
            Err(e) => self.errors.push(e),
        }
    }

    fn directiva(&mut self, no: usize, head: &str, head_tok: &Token, rest: &[Token]) {
        let (hs, he) = (head_tok.start, head_tok.end);
        match head {
            ".text" => {
                self.bind_pending_data_labels();
                self.seg = Seg::Text;
            }
            ".data" => self.seg = Seg::Data,
            ".globl" | ".global" | ".extern" => {}
            ".word" | ".half" | ".byte" => {
                if self.seg != Seg::Data {
                    self.error(no, hs, he, format!("'{head}' solo se puede usar dentro de .data"));
                    return;
                }
                let (dir, size) = match head {
                    ".word" => (DataDir::Word, 4),
                    ".half" => (DataDir::Half, 2),
                    _ => (DataDir::Byte, 1),
                };
                self.data_pc = self.data_pc.next_multiple_of(size);
                self.bind_pending_data_labels();
                let items = match self.parse_data_items(no, rest) {
                    Ok(it) => it,
                    Err(e) => {
                        self.errors.push(e);
                        return;
                    }
                };
                if items.is_empty() {
                    self.error(no, hs, he, format!("a '{head}' le faltan los valores"));
                    return;
                }
                let addr = self.data_pc;
                let total: u32 = items.iter().map(|i| i.count).sum();
                self.data_pc += total * size;
                self.stmts.push(Stmt { line: no, start: hs, end: he, addr, body: Body::Data { dir, items } });
            }
            ".space" => {
                if self.seg != Seg::Data {
                    self.error(no, hs, he, "'.space' solo se puede usar dentro de .data");
                    return;
                }
                self.bind_pending_data_labels();
                let n = match self.const_expr(no, rest) {
                    Ok(n) if n >= 0 => n as u32,
                    Ok(_) => {
                        self.error(no, hs, he, "'.space' necesita un tamaño positivo");
                        return;
                    }
                    Err(e) => {
                        self.errors.push(e);
                        return;
                    }
                };
                let addr = self.data_pc;
                self.data_pc += n;
                self.stmts.push(Stmt {
                    line: no,
                    start: hs,
                    end: he,
                    addr,
                    body: Body::Data { dir: DataDir::Bytes(vec![0; n as usize]), items: vec![] },
                });
            }
            ".string" | ".asciz" | ".ascii" => {
                if self.seg != Seg::Data {
                    self.error(no, hs, he, format!("'{head}' solo se puede usar dentro de .data"));
                    return;
                }
                self.bind_pending_data_labels();
                let mut bytes = Vec::new();
                let mut any = false;
                for t in rest {
                    match &t.tok {
                        Tok::Str(s) => {
                            any = true;
                            bytes.extend_from_slice(s.as_bytes());
                            if head != ".ascii" {
                                bytes.push(0);
                            }
                        }
                        Tok::Comma => {}
                        _ => {
                            self.error(no, t.start, t.end, "se esperaba un texto entre comillas");
                            return;
                        }
                    }
                }
                if !any {
                    self.error(no, hs, he, format!("a '{head}' le falta el texto entre comillas"));
                    return;
                }
                let addr = self.data_pc;
                self.data_pc += bytes.len() as u32;
                self.stmts.push(Stmt {
                    line: no,
                    start: hs,
                    end: he,
                    addr,
                    body: Body::Data { dir: DataDir::Bytes(bytes), items: vec![] },
                });
            }
            ".align" => {
                let n = match self.const_expr(no, rest) {
                    Ok(n) if (0..=12).contains(&n) => n as u32,
                    Ok(_) => {
                        self.error(no, hs, he, "'.align n' necesita n entre 0 y 12");
                        return;
                    }
                    Err(e) => {
                        self.errors.push(e);
                        return;
                    }
                };
                if self.seg == Seg::Data {
                    self.data_pc = self.data_pc.next_multiple_of(1 << n);
                }
            }
            ".macro" | ".end_macro" | ".include" | ".float" | ".double" | ".section" => {
                self.error(no, hs, he, format!("la directiva '{head}' todavía no está soportada en RISCode"));
            }
            _ => {
                self.error(no, hs, he, format!("directiva desconocida '{head}' (¿le falta ':' si es una etiqueta?)"));
            }
        }
    }

    fn parse_data_items(&self, no: usize, rest: &[Token]) -> Result<Vec<DataItem>, AsmError> {
        let mut items = Vec::new();
        for g in split_commas(rest) {
            if g.is_empty() {
                let pos = rest.last().map(|t| t.end).unwrap_or(0);
                return Err(err(no, pos, pos, "falta un valor entre comas"));
            }
            let (start, end) = (g[0].start, g[g.len() - 1].end);
            // "valor : cantidad"
            let (vals, count) = match g.iter().position(|t| t.tok == Tok::Colon) {
                Some(p) => {
                    let c = parse_expr(no, &g[p + 1..])?;
                    if c.sym.is_some() || c.val <= 0 {
                        return Err(err(no, start, end, "la cantidad después de ':' debe ser un número positivo"));
                    }
                    (&g[..p], c.val as u32)
                }
                None => (g, 1),
            };
            let expr = parse_expr(no, vals)?;
            items.push(DataItem { expr, count, start, end });
        }
        Ok(items)
    }

    fn const_expr(&self, no: usize, toks: &[Token]) -> Result<i64, AsmError> {
        let (s, e) = span(toks);
        let ex = parse_expr(no, toks)?;
        if toks.is_empty() {
            return Err(err(no, s, e, "falta un número"));
        }
        if ex.sym.is_some() {
            return Err(err(no, s, e, "se esperaba un número, no una etiqueta"));
        }
        Ok(ex.val)
    }

    fn instruccion(&mut self, no: usize, mn: &str, head: &Token, rest: &[Token]) {
        if !instr::es_mnemonico(mn) {
            let sug = sugerir(mn).map(|s| format!(" (¿quisiste decir '{s}'?)")).unwrap_or_default();
            self.error(no, head.start, head.end, format!("instrucción desconocida '{mn}'{sug}"));
            return;
        }
        if self.seg != Seg::Text {
            self.error(no, head.start, head.end, "hay una instrucción dentro de .data (¿falta '.text'?)");
            return;
        }
        let ops = match parse_operands(no, rest) {
            Ok(o) => o,
            Err(e) => {
                self.errors.push(e);
                return;
            }
        };
        let st = Stmt {
            line: no,
            start: head.start,
            end: head.end,
            addr: self.text_pc,
            body: Body::Instr { mn: mn.to_string(), ops },
        };
        match self.expandir(&st, false) {
            Ok(words) => {
                self.text_pc += 4 * words.len() as u32;
                self.stmts.push(st);
            }
            Err(e) => self.errors.push(e),
        }
    }

    // ═════════════════════ SEGUNDA PASADA ═════════════════════
    fn segunda_pasada(&mut self) -> Program {
        let mut prog = Program {
            data: vec![0; (self.data_pc - DATA_BASE) as usize],
            line_addr: vec![None; self.n_lines],
            ..Default::default()
        };
        let stmts = std::mem::take(&mut self.stmts);
        for st in &stmts {
            match &st.body {
                Body::Instr { .. } => match self.expandir(st, true) {
                    Ok(words) => {
                        if let Some(slot) = prog.line_addr.get_mut(st.line) {
                            slot.get_or_insert(st.addr);
                        }
                        for w in words {
                            prog.text.push(w);
                            prog.text_lines.push(st.line);
                        }
                    }
                    Err(e) => self.errors.push(e),
                },
                Body::Data { dir, items } => {
                    let mut off = (st.addr - DATA_BASE) as usize;
                    if let DataDir::Bytes(b) = dir {
                        prog.data[off..off + b.len()].copy_from_slice(b);
                        continue;
                    }
                    for it in items {
                        let v = match self.valor(st.line, &it.expr, it.start, it.end, true) {
                            Ok(v) => v,
                            Err(e) => {
                                self.errors.push(e);
                                continue;
                            }
                        };
                        let (size, ok) = match dir {
                            DataDir::Word => (4, (-(1i64 << 31)..(1i64 << 32)).contains(&v)),
                            DataDir::Half => (2, (-32768..65536).contains(&v)),
                            _ => (1, (-128..256).contains(&v)),
                        };
                        if !ok {
                            self.error(st.line, it.start, it.end, format!("el valor {v} no cabe en {size} byte(s)"));
                        }
                        let bytes = (v as u32).to_le_bytes();
                        for _ in 0..it.count {
                            prog.data[off..off + size].copy_from_slice(&bytes[..size]);
                            off += size;
                        }
                    }
                }
            }
        }
        prog.labels = std::mem::take(&mut self.labels);
        prog
    }

    fn valor(&self, line: usize, e: &Expr, start: usize, end: usize, fin: bool) -> Result<i64, AsmError> {
        match &e.sym {
            None => Ok(e.val),
            Some(name) => match self.labels.get(name) {
                Some(a) => Ok(*a as i64 + e.val),
                None if !fin => Ok(0),
                None => Err(err(line, start, end, format!("la etiqueta '{name}' no está definida"))),
            },
        }
    }

    /// Convierte una instrucción (o pseudoinstrucción) en una o más palabras de código máquina.
    /// En la primera pasada (`fin == false`) solo importa cuántas palabras salen.
    fn expandir(&self, st: &Stmt, fin: bool) -> Result<Vec<u32>, AsmError> {
        let Body::Instr { mn, ops } = &st.body else { return Ok(vec![]) };
        let mn = mn.as_str();
        let line = st.line;
        let pc = st.addr;
        let e_all = |msg: String| err(line, st.start, ops.last().map(|o| o.end).unwrap_or(st.end), msg);
        let uso = || {
            let syn = instr::documentacion(mn).map(|d| d.0).unwrap_or(mn);
            e_all(format!("operandos incorrectos, se usa así: {syn}"))
        };
        let n = ops.len();
        let reg = |i: usize| -> Result<u8, AsmError> {
            match ops.get(i) {
                Some(Op { kind: OpKind::Reg(r), .. }) => Ok(*r),
                Some(o) => Err(err(line, o.start, o.end, "aquí va un registro (por ejemplo t0, a0, sp)")),
                None => Err(uso()),
            }
        };
        // Número constante (sin etiquetas)
        let num = |i: usize| -> Result<i64, AsmError> {
            match ops.get(i) {
                Some(Op { kind: OpKind::Imm(Expr { sym: None, val }), .. }) => Ok(*val),
                Some(Op { kind: OpKind::Imm(Expr { sym: Some(s), .. }), start, end }) => {
                    Err(err(line, *start, *end, format!("aquí va un número, pero '{s}' no es una constante .eqv")))
                }
                Some(o) => Err(err(line, o.start, o.end, "aquí va un número")),
                None => Err(uso()),
            }
        };
        let imm12 = |i: usize| -> Result<i32, AsmError> {
            let v = num(i)?;
            if !fits_i12(v) {
                let o = &ops[i];
                return Err(err(line, o.start, o.end, format!("{v} no cabe en 12 bits (-2048..2047); usa 'li' a un registro")));
            }
            Ok(v as i32)
        };
        // Dirección de una etiqueta o expresión
        let target = |i: usize| -> Result<(i64, bool), AsmError> {
            match ops.get(i) {
                Some(Op { kind: OpKind::Imm(e), start, end }) => {
                    Ok((self.valor(line, e, *start, *end, fin)?, e.sym.is_some()))
                }
                Some(o) => Err(err(line, o.start, o.end, "aquí va una etiqueta")),
                None => Err(uso()),
            }
        };
        let branch_off = |i: usize| -> Result<i32, AsmError> {
            let (v, is_sym) = target(i)?;
            let off = if is_sym { v - pc as i64 } else { v };
            if fin && (!(-4096..=4094).contains(&off) || off % 2 != 0) {
                let o = &ops[i];
                return Err(err(line, o.start, o.end, format!("el salto condicional es muy largo ({off} bytes, máximo ±4 KiB); usa 'j' para saltos lejanos")));
            }
            Ok(off as i32)
        };
        let jal_off = |i: usize| -> Result<i32, AsmError> {
            let (v, is_sym) = target(i)?;
            let off = if is_sym { v - pc as i64 } else { v };
            if fin && (!(-(1 << 20)..(1 << 20)).contains(&off) || off % 2 != 0) {
                let o = &ops[i];
                return Err(err(line, o.start, o.end, format!("el salto es muy largo ({off} bytes, máximo ±1 MiB)")));
            }
            Ok(off as i32)
        };
        let mem = |i: usize| -> Result<(i32, u8), AsmError> {
            match ops.get(i) {
                Some(Op { kind: OpKind::Mem(e, base), start, end }) => {
                    let v = self.valor(line, e, *start, *end, fin)?;
                    if !fits_i12(v) {
                        return Err(err(line, *start, *end, format!("el desplazamiento {v} no cabe en 12 bits")));
                    }
                    Ok((v as i32, *base))
                }
                Some(o) => Err(err(line, o.start, o.end, "aquí va una dirección como 0(sp)")),
                None => Err(uso()),
            }
        };
        let csr = |i: usize| -> Result<u32, AsmError> {
            match ops.get(i) {
                Some(Op { kind: OpKind::Imm(Expr { sym: Some(s), val: 0 }), start, end }) => instr::csr_num(s)
                    .map(|c| c as u32)
                    .ok_or_else(|| err(line, *start, *end, format!("'{s}' no es un CSR conocido"))),
                Some(Op { kind: OpKind::Imm(Expr { sym: None, val }), start, end }) => {
                    if (0..4096).contains(val) {
                        Ok(*val as u32)
                    } else {
                        Err(err(line, *start, *end, "número de CSR inválido"))
                    }
                }
                Some(o) => Err(err(line, o.start, o.end, "aquí va un CSR (por ejemplo cycle, time, ustatus)")),
                None => Err(uso()),
            }
        };
        let expect = |k: usize| -> Result<(), AsmError> { if n == k { Ok(()) } else { Err(uso()) } };
        // auipc + instrucción con la parte baja: dirección relativa al pc
        let pcrel = |v: i64| hi_lo(v - pc as i64);

        // ───── instrucciones reales ─────
        if let Some(ins) = instr::buscar_instruccion(mn) {
            let (op, f3, f7) = (ins.opcode, ins.funct3, ins.funct7);
            return Ok(match ins.formato {
                Formato::R => {
                    expect(3)?;
                    if let Some(Op { kind: OpKind::Imm(_), start, end }) = ops.get(2) {
                        return Err(err(line, *start, *end, format!("'{mn}' usa 3 registros; para un número usa '{mn}i'")));
                    }
                    vec![enc_r(f7, reg(2)?, reg(1)?, f3, reg(0)?, op)]
                }
                Formato::I => {
                    expect(3)?;
                    vec![enc_i(imm12(2)?, reg(1)?, f3, reg(0)?, op)]
                }
                Formato::IShift => {
                    expect(3)?;
                    let sh = num(2)?;
                    if !(0..32).contains(&sh) {
                        let o = &ops[2];
                        return Err(err(line, o.start, o.end, "el desplazamiento debe estar entre 0 y 31"));
                    }
                    vec![enc_i(((f7 << 5) | sh as u32) as i32, reg(1)?, f3, reg(0)?, op)]
                }
                Formato::ILoad => {
                    let rd = reg(0)?;
                    match (n, ops.get(1).map(|o| &o.kind)) {
                        (2, Some(OpKind::Mem(..))) => {
                            let (off, base) = mem(1)?;
                            vec![enc_i(off, base, f3, rd, op)]
                        }
                        (2, Some(OpKind::Imm(e))) => {
                            let (v, _) = target(1)?;
                            if e.sym.is_some() {
                                let (hi, lo) = pcrel(v);
                                vec![enc_u(hi, rd, 0x17), enc_i(lo, rd, f3, rd, op)]
                            } else if fits_i12(v) {
                                vec![enc_i(v as i32, 0, f3, rd, op)]
                            } else {
                                let (hi, lo) = hi_lo(v);
                                vec![enc_u(hi, rd, 0x37), enc_i(lo, rd, f3, rd, op)]
                            }
                        }
                        _ => return Err(uso()),
                    }
                }
                Formato::S => {
                    let rs2 = reg(0)?;
                    match (n, ops.get(1).map(|o| &o.kind)) {
                        (2, Some(OpKind::Mem(..))) => {
                            let (off, base) = mem(1)?;
                            vec![enc_s(off, rs2, base, f3, op)]
                        }
                        (2 | 3, Some(OpKind::Imm(e))) => {
                            let (v, _) = target(1)?;
                            if e.sym.is_none() && fits_i12(v) {
                                vec![enc_s(v as i32, rs2, 0, f3, op)]
                            } else {
                                if n != 3 {
                                    return Err(e_all(format!(
                                        "para guardar en una etiqueta o dirección grande se necesita un registro temporal: {mn} {}, {}, t0",
                                        crate::lexer::ABI_NAMES[rs2 as usize],
                                        e.sym.clone().unwrap_or_else(|| format!("{v:#x}"))
                                    )));
                                }
                                let tmp = reg(2)?;
                                if e.sym.is_some() {
                                    let (hi, lo) = pcrel(v);
                                    vec![enc_u(hi, tmp, 0x17), enc_s(lo, rs2, tmp, f3, op)]
                                } else {
                                    let (hi, lo) = hi_lo(v);
                                    vec![enc_u(hi, tmp, 0x37), enc_s(lo, rs2, tmp, f3, op)]
                                }
                            }
                        }
                        _ => return Err(uso()),
                    }
                }
                Formato::SB => {
                    expect(3)?;
                    vec![enc_b(branch_off(2)?, reg(1)?, reg(0)?, f3)]
                }
                Formato::U => {
                    expect(2)?;
                    let v = num(1)?;
                    if !(-(1 << 19)..(1 << 20)).contains(&v) {
                        let o = &ops[1];
                        return Err(err(line, o.start, o.end, "el inmediato de lui/auipc es de 20 bits (0..0xFFFFF)"));
                    }
                    vec![enc_u(v as u32, reg(0)?, op)]
                }
                Formato::UJ => match n {
                    1 => vec![enc_j(jal_off(0)?, RA)],
                    2 => vec![enc_j(jal_off(1)?, reg(0)?)],
                    _ => return Err(uso()),
                },
                Formato::IJalr => match (n, ops.get(1).map(|o| &o.kind)) {
                    (1, _) => vec![enc_i(0, reg(0)?, 0, RA, op)],
                    (2, Some(OpKind::Mem(..))) => {
                        let (off, base) = mem(1)?;
                        vec![enc_i(off, base, 0, reg(0)?, op)]
                    }
                    (2, Some(OpKind::Reg(_))) => vec![enc_i(0, reg(1)?, 0, reg(0)?, op)],
                    (3, _) => vec![enc_i(imm12(2)?, reg(1)?, 0, reg(0)?, op)],
                    _ => return Err(uso()),
                },
                Formato::Sistema => match mn {
                    "ecall" => vec![0x0000_0073],
                    "ebreak" => vec![0x0010_0073],
                    "fence.i" => vec![0x0000_100F],
                    // fence [pred, succ] (en RARS: fence 1, 1)
                    _ if n == 2 => {
                        let (p, s) = (num(0)?, num(1)?);
                        if !(0..16).contains(&p) || !(0..16).contains(&s) {
                            return Err(uso());
                        }
                        vec![((p as u32) << 24) | ((s as u32) << 20) | 0x0F]
                    }
                    _ => vec![0x0FF0_000F],
                },
                Formato::Csr => {
                    expect(3)?;
                    vec![enc_i(csr(1)? as i32, reg(2)?, f3, reg(0)?, op)]
                }
                Formato::CsrI => {
                    expect(3)?;
                    let u = num(2)?;
                    if !(0..32).contains(&u) {
                        let o = &ops[2];
                        return Err(err(line, o.start, o.end, "el inmediato debe estar entre 0 y 31"));
                    }
                    vec![enc_i(csr(1)? as i32, u as u8, f3, reg(0)?, op)]
                }
            });
        }

        // ───── pseudoinstrucciones ─────
        let addi = |rd: u8, rs: u8, imm: i32| enc_i(imm, rs, ADDI.1, rd, ADDI.0);
        let b = |f3: u32, rs1: u8, rs2: u8, off: i32| enc_b(off, rs2, rs1, f3);
        Ok(match mn {
            "nop" => {
                expect(0)?;
                vec![addi(0, 0, 0)]
            }
            "li" => {
                expect(2)?;
                let rd = reg(0)?;
                let v = num(1)?;
                if !(-(1i64 << 31)..(1i64 << 32)).contains(&v) {
                    let o = &ops[1];
                    return Err(err(line, o.start, o.end, format!("{v} no cabe en 32 bits")));
                }
                let v32 = v as u32 as i32 as i64;
                if fits_i12(v32) {
                    vec![addi(rd, 0, v32 as i32)]
                } else {
                    let (hi, lo) = hi_lo(v32);
                    vec![enc_u(hi, rd, 0x37), addi(rd, rd, lo)]
                }
            }
            "la" => {
                expect(2)?;
                let rd = reg(0)?;
                let (v, is_sym) = target(1)?;
                if is_sym {
                    let (hi, lo) = pcrel(v);
                    vec![enc_u(hi, rd, 0x17), addi(rd, rd, lo)]
                } else {
                    let (hi, lo) = hi_lo(v);
                    vec![enc_u(hi, rd, 0x37), addi(rd, rd, lo)]
                }
            }
            "mv" => {
                expect(2)?;
                // RARS: mv rd, rs = add rd, x0, rs
                vec![enc_r(0, reg(1)?, 0, 0, reg(0)?, 0x33)]
            }
            "not" => {
                expect(2)?;
                vec![enc_i(-1, reg(1)?, 0b100, reg(0)?, 0x13)]
            }
            "neg" => {
                expect(2)?;
                vec![enc_r(0x20, reg(1)?, 0, 0, reg(0)?, 0x33)]
            }
            "seqz" => {
                expect(2)?;
                vec![enc_i(1, reg(1)?, 0b011, reg(0)?, 0x13)]
            }
            "snez" => {
                expect(2)?;
                vec![enc_r(0, reg(1)?, 0, 0b011, reg(0)?, 0x33)]
            }
            "sltz" => {
                expect(2)?;
                vec![enc_r(0, 0, reg(1)?, 0b010, reg(0)?, 0x33)]
            }
            "sgtz" => {
                expect(2)?;
                vec![enc_r(0, reg(1)?, 0, 0b010, reg(0)?, 0x33)]
            }
            "j" => {
                expect(1)?;
                vec![enc_j(jal_off(0)?, 0)]
            }
            "jr" => {
                expect(1)?;
                vec![enc_i(0, reg(0)?, 0, 0, 0x67)]
            }
            "ret" => {
                expect(0)?;
                vec![enc_i(0, RA, 0, 0, 0x67)]
            }
            "call" | "tail" => {
                expect(1)?;
                let (v, _) = target(0)?;
                let (hi, lo) = pcrel(v);
                // Igual que RARS: el temporal es t1 (x6) en los dos casos
                let (tmp, rd) = if mn == "call" { (6, RA) } else { (6, 0) };
                vec![enc_u(hi, tmp, 0x17), enc_i(lo, tmp, 0, rd, 0x67)]
            }
            "beqz" | "bnez" | "blez" | "bgez" | "bltz" | "bgtz" => {
                expect(2)?;
                let rs = reg(0)?;
                let off = branch_off(1)?;
                vec![match mn {
                    "beqz" => b(branch_f3("beq"), rs, 0, off),
                    "bnez" => b(branch_f3("bne"), rs, 0, off),
                    "blez" => b(branch_f3("bge"), 0, rs, off),
                    "bgez" => b(branch_f3("bge"), rs, 0, off),
                    "bltz" => b(branch_f3("blt"), rs, 0, off),
                    _ => b(branch_f3("blt"), 0, rs, off),
                }]
            }
            "bgt" | "ble" | "bgtu" | "bleu" => {
                expect(3)?;
                let (r1, r2) = (reg(0)?, reg(1)?);
                let off = branch_off(2)?;
                let real = match mn {
                    "bgt" => "blt",
                    "ble" => "bge",
                    "bgtu" => "bltu",
                    _ => "bgeu",
                };
                vec![b(branch_f3(real), r2, r1, off)]
            }
            "csrr" => {
                expect(2)?;
                vec![enc_i(csr(1)? as i32, 0, 0b010, reg(0)?, 0x73)]
            }
            "csrw" | "csrs" | "csrc" => {
                expect(2)?;
                let f3 = match mn {
                    "csrw" => 0b001,
                    "csrs" => 0b010,
                    _ => 0b011,
                };
                // Se aceptan los dos órdenes: "csrw ustatus, t0" (estándar) y "csrw t0, ustatus" (RARS)
                let (c, r) = if matches!(ops[0].kind, OpKind::Reg(_)) { (csr(1)?, reg(0)?) } else { (csr(0)?, reg(1)?) };
                vec![enc_i(c as i32, r, f3, 0, 0x73)]
            }
            "csrwi" | "csrsi" | "csrci" => {
                expect(2)?;
                let f3 = match mn {
                    "csrwi" => 0b101,
                    "csrsi" => 0b110,
                    _ => 0b111,
                };
                let u = num(1)?;
                if !(0..32).contains(&u) {
                    return Err(uso());
                }
                vec![enc_i(csr(0)? as i32, u as u8, f3, 0, 0x73)]
            }
            "rdcycle" | "rdtime" | "rdinstret" => {
                expect(1)?;
                let c = match mn {
                    "rdcycle" => 0xC00,
                    "rdtime" => 0xC01,
                    _ => 0xC02,
                };
                vec![enc_i(c, 0, 0b010, reg(0)?, 0x73)]
            }
            _ => return Err(e_all(format!("'{mn}' todavía no está implementada"))),
        })
    }
}

fn span(toks: &[Token]) -> (usize, usize) {
    match (toks.first(), toks.last()) {
        (Some(a), Some(b)) => (a.start, b.end),
        _ => (0, 0),
    }
}

/// Separa los operandos. Como en RARS, las comas son opcionales: `addi sp, sp -4` también vale.
/// Dos términos seguidos (registro, número, etiqueta o `)`) empiezan un operando nuevo, y un `-`
/// con espacio antes y pegado a un número (` -4`) también.
fn split_commas(toks: &[Token]) -> Vec<&[Token]> {
    if toks.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    for g in toks.split(|t| t.tok == Tok::Comma) {
        let mut begin = 0;
        for k in 1..g.len() {
            let prev = &g[k - 1];
            let t = &g[k];
            let prev_ends_term = matches!(prev.tok, Tok::Int(_) | Tok::Ident(_) | Tok::RParen);
            let starts_new = match &t.tok {
                Tok::Int(_) | Tok::Ident(_) => prev_ends_term,
                Tok::Minus | Tok::Plus => {
                    prev_ends_term
                        && t.start > prev.end
                        && g.get(k + 1).is_some_and(|n| n.start == t.end && matches!(n.tok, Tok::Int(_)))
                }
                _ => false,
            };
            if starts_new {
                out.push(&g[begin..k]);
                begin = k;
            }
        }
        out.push(&g[begin..]);
    }
    out
}

fn parse_operands(no: usize, toks: &[Token]) -> Result<Vec<Op>, AsmError> {
    let mut out = Vec::new();
    for (k, g) in split_commas(toks).into_iter().enumerate() {
        if g.is_empty() {
            let pos = if k == 0 { toks[0].start } else { toks.last().map(|t| t.end).unwrap_or(0) };
            return Err(err(no, pos, pos + 1, "falta un operando (¿hay una coma de más?)"));
        }
        out.push(parse_operand(no, g)?);
    }
    Ok(out)
}

fn parse_operand(no: usize, g: &[Token]) -> Result<Op, AsmError> {
    let (start, end) = span(g);
    if g.len() == 1 {
        if let Tok::Ident(n) = &g[0].tok {
            if let Some(r) = reg_num(n) {
                return Ok(Op { kind: OpKind::Reg(r), start, end });
            }
        }
    }
    if g.last().map(|t| &t.tok) == Some(&Tok::RParen) {
        let Some(p) = g.iter().rposition(|t| t.tok == Tok::LParen) else {
            return Err(err(no, start, end, "falta abrir el paréntesis '('"));
        };
        let inner = &g[p + 1..g.len() - 1];
        let base = match inner {
            [Token { tok: Tok::Ident(n), .. }] => reg_num(n),
            _ => None,
        };
        let Some(base) = base else {
            return Err(err(no, start, end, "dentro de los paréntesis va un registro, por ejemplo 0(sp)"));
        };
        let e = parse_expr(no, &g[..p])?;
        return Ok(Op { kind: OpKind::Mem(e, base), start, end });
    }
    let e = parse_expr(no, g)?;
    Ok(Op { kind: OpKind::Imm(e), start, end })
}

/// Expresión simple: números y como máximo una etiqueta, con + y -. Ej: `etiqueta+4`, `-12`.
fn parse_expr(no: usize, toks: &[Token]) -> Result<Expr, AsmError> {
    let mut e = Expr { sym: None, val: 0 };
    let mut sign: i64 = 1;
    let mut expecting_term = true;
    for t in toks {
        match (&t.tok, expecting_term) {
            (Tok::Plus, true) => {}
            (Tok::Minus, true) => sign = -sign,
            (Tok::Plus, false) => {
                sign = 1;
                expecting_term = true;
            }
            (Tok::Minus, false) => {
                sign = -1;
                expecting_term = true;
            }
            (Tok::Int(v), true) => {
                e.val = e.val.wrapping_add(sign * v);
                expecting_term = false;
            }
            (Tok::Ident(n), true) => {
                if reg_num(n).is_some() {
                    return Err(err(no, t.start, t.end, format!("'{n}' es un registro y aquí no va un registro")));
                }
                if e.sym.is_some() || sign < 0 {
                    return Err(err(no, t.start, t.end, "solo se puede sumar una etiqueta con números"));
                }
                e.sym = Some(n.clone());
                expecting_term = false;
            }
            (Tok::Str(_), _) => return Err(err(no, t.start, t.end, "aquí no va un texto")),
            _ => {
                let msg = if expecting_term { "se esperaba un número o etiqueta" } else { "falta una coma" };
                return Err(err(no, t.start, t.end, msg));
            }
        }
    }
    if expecting_term && !toks.is_empty() {
        let (s, en) = span(toks);
        return Err(err(no, s, en, "la expresión está incompleta"));
    }
    Ok(e)
}

/// Sugiere la instrucción más parecida (distancia de edición 1 o 2).
fn sugerir(mn: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    let all = instr::INSTRUCCIONES.iter().map(|i| i.nombre).chain(instr::PSEUDO.iter().copied());
    for cand in all {
        let d = levenshtein(mn, cand);
        if d <= 2 && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((cand, d));
        }
    }
    best.map(|b| b.0)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asm(src: &str) -> Vec<u32> {
        assemble(src).unwrap_or_else(|e| panic!("{e:?}")).text
    }

    #[test]
    fn basicas() {
        assert_eq!(asm("li t0, 0"), vec![0x00000293]);
        assert_eq!(asm("add t0, t1, t2"), vec![0x007302b3]);
        assert_eq!(asm("addi sp, sp, -4"), vec![0xffc10113]);
        assert_eq!(asm("sw ra, 0(sp)"), vec![0x00112023]);
        assert_eq!(asm("lw ra, 0(sp)"), vec![0x00012083]);
        assert_eq!(asm("ecall"), vec![0x00000073]);
        assert_eq!(asm("jr ra"), vec![0x00008067]);
    }

    #[test]
    fn pseudo_li_grande() {
        // li t0, 0x12345678 -> lui t0, 0x12345 ; addi t0, t0, 0x678
        assert_eq!(asm("li t0, 0x12345678"), vec![0x123452b7, 0x67828293]);
        // con parte baja negativa
        let w = asm("li a0, 0xFFFF0004");
        assert_eq!(w, vec![0xffff0537, 0x00450513]);
    }

    #[test]
    fn saltos_y_etiquetas() {
        let src = "main:\n beq t0, t1, fin\n j main\nfin:\n nop";
        let w = asm(src);
        assert_eq!(w.len(), 3);
        assert_eq!(w[0], 0x00628463); // beq t0,t1,+8
        assert_eq!(w[1], 0xffdff06f); // jal x0,-4
    }

    #[test]
    fn datos() {
        let p = assemble(".data\na: .byte 1\nb: .word 0x11223344\n.text\nla t0, b").unwrap();
        assert_eq!(p.labels["a"], DATA_BASE);
        assert_eq!(p.labels["b"], DATA_BASE + 4);
        assert_eq!(&p.data[4..8], &[0x44, 0x33, 0x22, 0x11]);
    }

    #[test]
    fn comas_opcionales() {
        assert_eq!(asm("addi sp, sp -20"), asm("addi sp, sp, -20"));
        assert_eq!(asm("add t0 t1 t2"), asm("add t0, t1, t2"));
        assert_eq!(asm("lw t0 4(sp)"), asm("lw t0, 4(sp)"));
        let p = assemble(".data\nx: .word 1 2 3\ny: .word 7:3").unwrap();
        assert_eq!(p.data.len(), 24);
    }

    #[test]
    fn eqv() {
        let w = asm(".eqv N, 5\n.eqv M 7\nli t0, N\naddi t1, t1, M");
        assert_eq!(w, vec![0x00500293, 0x00730313]);
    }

    #[test]
    fn errores_utiles() {
        let e = assemble("addd t0, t1, t2").unwrap_err();
        assert!(e[0].msg.contains("add"), "{}", e[0].msg);
        let e = assemble("j noexiste").unwrap_err();
        assert!(e[0].msg.contains("noexiste"));
        // errores de las dos pasadas juntos
        let e = assemble("addd t0, t1, t2\nj noexiste").unwrap_err();
        assert_eq!(e.len(), 2);
        let e = assemble("a:\na:").unwrap_err();
        assert!(e[0].msg.contains("ya está definida"));
    }
}
