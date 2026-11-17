//! Lexer: parte cada línea del .asm en tokens.
//!
//! Igual que `trim` / `strip_comment` del Proyecto 3, pero devolviendo tokens con su
//! posición (columna) para poder subrayar errores en el editor.

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Str(String),
    Comma,
    LParen,
    RParen,
    Colon,
    Plus,
    Minus,
    Other(char),
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    /// Columna (en bytes) donde empieza el token dentro de la línea.
    pub start: usize,
    /// Columna (en bytes) donde termina el token.
    pub end: usize,
}

/// Quita el comentario (`#` fuera de comillas) de una línea.
pub fn strip_comment(line: &str) -> &str {
    let mut in_str = false;
    let mut in_char = false;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        match c {
            '\\' if in_str || in_char => escape = true,
            '"' if !in_char => in_str = !in_str,
            '\'' if !in_str => in_char = !in_char,
            '#' if !in_str && !in_char => return &line[..i],
            _ => {}
        }
    }
    line
}

pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '.' || c == '$'
}

pub fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$'
}

/// Error del lexer: (columna inicio, columna fin, mensaje).
pub type LexError = (usize, usize, String);

pub fn tokenize(line: &str) -> Result<Vec<Token>, LexError> {
    let bytes: Vec<(usize, char)> = line.char_indices().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let end_of = |k: usize| -> usize { bytes.get(k).map(|b| b.0).unwrap_or(line.len()) };
    while i < bytes.len() {
        let (pos, c) = bytes[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let single = |t: Tok| Token { tok: t, start: pos, end: pos + c.len_utf8() };
        match c {
            ',' => {
                out.push(single(Tok::Comma));
                i += 1;
            }
            '(' => {
                out.push(single(Tok::LParen));
                i += 1;
            }
            ')' => {
                out.push(single(Tok::RParen));
                i += 1;
            }
            ':' => {
                out.push(single(Tok::Colon));
                i += 1;
            }
            '+' => {
                out.push(single(Tok::Plus));
                i += 1;
            }
            '-' => {
                out.push(single(Tok::Minus));
                i += 1;
            }
            '"' => {
                let mut s = String::new();
                let mut j = i + 1;
                let mut closed = false;
                while j < bytes.len() {
                    let ch = bytes[j].1;
                    if ch == '\\' && j + 1 < bytes.len() {
                        s.push(unescape(bytes[j + 1].1));
                        j += 2;
                        continue;
                    }
                    if ch == '"' {
                        closed = true;
                        j += 1;
                        break;
                    }
                    s.push(ch);
                    j += 1;
                }
                if !closed {
                    return Err((pos, line.len(), "falta cerrar las comillas \"".into()));
                }
                out.push(Token { tok: Tok::Str(s), start: pos, end: end_of(j) });
                i = j;
            }
            '\'' => {
                // Carácter: 'a' o '\n'
                let (val, len) = match (bytes.get(i + 1), bytes.get(i + 2), bytes.get(i + 3)) {
                    (Some((_, '\\')), Some((_, e)), Some((_, '\''))) => (unescape(*e), 4),
                    (Some((_, ch)), Some((_, '\'')), _) => (*ch, 3),
                    _ => return Err((pos, end_of(i + 1), "carácter mal escrito, usa 'a'".into())),
                };
                out.push(Token { tok: Tok::Int(val as i64), start: pos, end: end_of(i + len) });
                i += len;
            }
            c if c.is_ascii_digit() => {
                let mut j = i;
                while j < bytes.len() && (bytes[j].1.is_ascii_alphanumeric() || bytes[j].1 == '_') {
                    j += 1;
                }
                let text = &line[pos..end_of(j)];
                match parse_number(text) {
                    Some(v) => out.push(Token { tok: Tok::Int(v), start: pos, end: end_of(j) }),
                    None => return Err((pos, end_of(j), format!("número inválido: {text}"))),
                }
                i = j;
            }
            c if is_ident_start(c) => {
                let mut j = i;
                while j < bytes.len() && is_ident_char(bytes[j].1) {
                    j += 1;
                }
                out.push(Token {
                    tok: Tok::Ident(line[pos..end_of(j)].to_string()),
                    start: pos,
                    end: end_of(j),
                });
                i = j;
            }
            other => {
                out.push(single(Tok::Other(other)));
                i += 1;
            }
        }
    }
    Ok(out)
}

fn unescape(c: char) -> char {
    match c {
        'n' => '\n',
        't' => '\t',
        'r' => '\r',
        '0' => '\0',
        other => other,
    }
}

/// Decimal, hexadecimal (0x) o binario (0b). Como `parse_literal` del Proyecto 3.
pub fn parse_number(text: &str) -> Option<i64> {
    let t = text.replace('_', "");
    let lower = t.to_ascii_lowercase();
    if let Some(h) = lower.strip_prefix("0x") {
        i64::from_str_radix(h, 16).ok()
    } else if let Some(b) = lower.strip_prefix("0b") {
        i64::from_str_radix(b, 2).ok()
    } else {
        lower.parse::<i64>().ok()
    }
}

/// Convierte un nombre de registro (ABI o xN) a su número. Como `reg_abi_to_num`.
pub fn reg_num(name: &str) -> Option<u8> {
    let n = name.to_ascii_lowercase();
    if let Some(rest) = n.strip_prefix('x') {
        if let Ok(v) = rest.parse::<u8>() {
            if v < 32 && (rest.len() == 1 || !rest.starts_with('0')) {
                return Some(v);
            }
        }
    }
    const ABI: [&str; 32] = [
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
        "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
        "t5", "t6",
    ];
    if n == "fp" {
        return Some(8);
    }
    ABI.iter().position(|r| *r == n).map(|p| p as u8)
}

pub const ABI_NAMES: [&str; 32] = [
    "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
    "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
    "t5", "t6",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comentarios() {
        assert_eq!(strip_comment("li a0, 1 # hola"), "li a0, 1 ");
        assert_eq!(strip_comment(".string \"a#b\" # c"), ".string \"a#b\" ");
    }

    #[test]
    fn tokens() {
        let t = tokenize("sw t0, -4(sp)").unwrap();
        assert_eq!(t.len(), 8);
        assert_eq!(t[2].tok, Tok::Comma);
        assert_eq!(t[3].tok, Tok::Minus);
        assert_eq!(t[4].tok, Tok::Int(4));
        let t = tokenize("levelC:.word 0xFF").unwrap();
        assert_eq!(t[0].tok, Tok::Ident("levelC".into()));
        assert_eq!(t[1].tok, Tok::Colon);
        assert_eq!(t[2].tok, Tok::Ident(".word".into()));
        assert_eq!(t[3].tok, Tok::Int(255));
    }

    #[test]
    fn registros() {
        assert_eq!(reg_num("t0"), Some(5));
        assert_eq!(reg_num("x31"), Some(31));
        assert_eq!(reg_num("fp"), Some(8));
        assert_eq!(reg_num("x32"), None);
        assert_eq!(reg_num("hola"), None);
    }
}
