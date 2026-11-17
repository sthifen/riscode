//! Colores del editor: clasifica cada pedazo de una línea (instrucción, registro, número…).
//! Es tolerante: nunca falla, aunque la línea tenga errores.

use std::collections::HashSet;
use std::ops::Range;

use crate::instr;
use crate::lexer::{is_ident_char, is_ident_start, reg_num};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Class {
    Text,
    Comment,
    Instruction,
    Pseudo,
    Directive,
    Register,
    Number,
    String,
    LabelDef,
    LabelRef,
    Constant,
    Punct,
    Unknown,
}

/// Nombres que hay en el archivo: etiquetas definidas y constantes `.eqv`.
#[derive(Default, Clone)]
pub struct Symbols {
    pub labels: HashSet<String>,
    pub eqvs: HashSet<String>,
}

pub fn scan_symbols(src: &str) -> Symbols {
    let mut s = Symbols::default();
    for line in src.lines() {
        let code = crate::lexer::strip_comment(line);
        let t = code.trim_start();
        if let Some(rest) = t.strip_prefix(".eqv") {
            let name: String = rest.trim_start().chars().take_while(|c| is_ident_char(*c)).collect();
            if !name.is_empty() {
                s.eqvs.insert(name);
            }
            continue;
        }
        // etiquetas al inicio: "a: b: instr"
        let mut rest = t;
        loop {
            let name: String = rest.chars().take_while(|c| is_ident_char(*c)).collect();
            if name.is_empty() {
                break;
            }
            let after = rest[name.len()..].trim_start();
            if let Some(r) = after.strip_prefix(':') {
                s.labels.insert(name);
                rest = r.trim_start();
            } else {
                break;
            }
        }
    }
    s
}

pub fn highlight_line(line: &str, syms: &Symbols) -> Vec<(Range<usize>, Class)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let end_of = |k: usize| chars.get(k).map(|c| c.0).unwrap_or(line.len());
    let mut i = 0;
    let mut first_word = true;
    while i < chars.len() {
        let (pos, c) = chars[i];
        if c == '#' {
            out.push((pos..line.len(), Class::Comment));
            break;
        }
        if c.is_whitespace() {
            let mut j = i;
            while j < chars.len() && chars[j].1.is_whitespace() {
                j += 1;
            }
            out.push((pos..end_of(j), Class::Text));
            i = j;
            continue;
        }
        if c == '"' || c == '\'' {
            let mut j = i + 1;
            while j < chars.len() {
                if chars[j].1 == '\\' {
                    j += 2;
                    continue;
                }
                if chars[j].1 == c {
                    j += 1;
                    break;
                }
                j += 1;
            }
            let j = j.min(chars.len());
            let class = if c == '"' { Class::String } else { Class::Number };
            out.push((pos..end_of(j), class));
            i = j;
            continue;
        }
        if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(|n| n.1.is_ascii_digit()) && !first_word) {
            let mut j = i + 1;
            while j < chars.len() && (chars[j].1.is_ascii_alphanumeric() || chars[j].1 == '_') {
                j += 1;
            }
            out.push((pos..end_of(j), Class::Number));
            i = j;
            continue;
        }
        if is_ident_start(c) {
            let mut j = i;
            while j < chars.len() && is_ident_char(chars[j].1) {
                j += 1;
            }
            let word = &line[pos..end_of(j)];
            // ¿es una etiqueta "nombre:"?
            let mut k = j;
            while k < chars.len() && chars[k].1 == ' ' {
                k += 1;
            }
            let is_def = chars.get(k).is_some_and(|c| c.1 == ':');
            let lower = word.to_ascii_lowercase();
            let class = if is_def {
                Class::LabelDef
            } else if first_word {
                first_word = false;
                if lower.starts_with('.') {
                    if instr::DIRECTIVAS.contains(&lower.as_str()) { Class::Directive } else { Class::Unknown }
                } else if instr::es_pseudo(&lower) {
                    Class::Pseudo
                } else if instr::es_mnemonico(&lower) {
                    Class::Instruction
                } else {
                    Class::Unknown
                }
            } else if reg_num(word).is_some() {
                Class::Register
            } else if syms.eqvs.contains(word) {
                Class::Constant
            } else if instr::csr_num(&lower).is_some() {
                Class::Register
            } else {
                Class::LabelRef
            };
            out.push((pos..end_of(j), class));
            i = j;
            continue;
        }
        out.push((pos..pos + c.len_utf8(), Class::Punct));
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colores() {
        let syms = scan_symbols(".eqv N 3\nmain: li t0, N # hola");
        assert!(syms.labels.contains("main"));
        assert!(syms.eqvs.contains("N"));
        let h = highlight_line("main: li t0, N # hola", &syms);
        let classes: Vec<Class> = h.iter().filter(|(_, c)| *c != Class::Text).map(|(_, c)| *c).collect();
        assert_eq!(
            classes,
            vec![Class::LabelDef, Class::Punct, Class::Pseudo, Class::Register, Class::Punct, Class::Constant, Class::Comment]
        );
    }
}
