// vim: ts=4:sw=4:expandtab:
/*
 * Copyright (c) 2026 Travis Geiselbrecht
 *
 * Use of this source code is governed by a MIT-style
 * license that can be found in the LICENSE file or at
 * https://opensource.org/licenses/MIT
 */
//! Holds a disassembler to an ASxxxx test file: the instruction-per-line
//! files the distribution ships for each assembler (`t6809.asm`), every
//! line an instruction and, in its comment, the bytes the assembler makes
//! of it. Each line's bytes are disassembled at the line's address and the
//! text compared with the source after both are normalised -- case and
//! spacing, every number and symbol to its value, `.` to the address --
//! so the comparison is of what the two say, not how they spell it.

/// One instruction line.
pub struct Case {
    pub line: usize,
    pub addr: u16,
    pub bytes: Vec<u8>,
    pub source: String,
}

/// The comment's bytes, `8e 00 05`, `d9*02` or `CDr28s00` (the `r` and `s`
/// marking a relocated byte); None if it is not all hex.
fn comment_bytes(comment: &str) -> Option<Vec<u8>> {
    let digits: String = comment.chars().filter(|c| !c.is_whitespace() && !matches!(c, '*' | 'r' | 's')).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok()).collect()
}

/// Every instruction line of `text`, addressed from 0 by the bytes before
/// it, and every `label:` at the address it labels. Directives, comment
/// lines, lines with no bytes and lines the assembler flagged are not cases;
/// `skip` drops more by their source text.
fn walk(text: &str, skip: &dyn Fn(&str) -> bool) -> (Vec<Case>, Vec<(String, i64)>) {
    let mut out = Vec::new();
    let mut labels = Vec::new();
    let mut addr: u16 = 0;
    for (i, raw) in text.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('.') {
            continue;
        }
        let (code, comment) = trimmed.split_once(';').unwrap_or((trimmed, ""));
        let code = code.trim();
        let code = match code.split_once(':') {
            Some((label, rest)) if !label.contains(char::is_whitespace) => {
                labels.push((label.to_ascii_lowercase(), addr as i64));
                rest.trim()
            }
            _ => code,
        };
        if code.is_empty() || code.contains('=') {
            continue;
        }
        // `;o ; 10`: the assembler's error flags ahead of the bytes, on an
        // instruction it took exception to -- still bytes, never a case
        let (flagged, comment) = match comment.rsplit_once(';') {
            Some((_, bytes)) => (true, bytes),
            None => (false, comment),
        };
        let Some(bytes) = comment_bytes(comment) else {
            continue;
        };
        let n = bytes.len() as u16;
        if !flagged && !skip(code) {
            out.push(Case { line: i + 1, addr, bytes, source: code.to_string() });
        }
        addr = addr.wrapping_add(n);
    }
    (out, labels)
}

pub fn cases(text: &str, skip: &dyn Fn(&str) -> bool) -> Vec<Case> {
    walk(text, skip).0
}

/// The file's `name = value` definitions, a plain number on the right, and
/// its labels.
pub fn symbols(text: &str) -> Vec<(String, i64)> {
    let mut defs: Vec<(String, i64)> = text
        .lines()
        .filter_map(|l| {
            let code = l.split(';').next()?;
            let (name, value) = code.split_once('=')?;
            let value = value.trim().to_ascii_lowercase();
            let v = match value.strip_prefix("0x") {
                Some(h) => i64::from_str_radix(h, 16).ok()?,
                None => value.parse().ok()?,
            };
            Some((name.trim().to_ascii_lowercase(), v))
        })
        .collect();
    defs.extend(walk(text, &|_| false).1);
    defs
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Word(String),
    Num(i64),
    Punct(char),
    Space,
}

fn tokenize(s: &str, here: u16, symbols: &[(String, i64)]) -> Vec<Tok> {
    let s = s.to_ascii_lowercase();
    let chars: Vec<char> = s.chars().collect();
    let mut toks = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            toks.push(Tok::Space);
        } else if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_alphanumeric() {
                i += 1;
            }
            let lit: String = chars[start..i].iter().collect();
            let v = if let Some(h) = lit.strip_prefix("0x").or_else(|| lit.strip_prefix("0h")) {
                i64::from_str_radix(h, 16)
            } else if let Some(o) = lit.strip_prefix("0o") {
                i64::from_str_radix(o, 8)
            } else {
                lit.parse()
            };
            toks.push(v.map_or(Tok::Word(lit), Tok::Num));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '\'') {
                i += 1;
            }
            let w: String = chars[start..i].iter().collect();
            match symbols.iter().find(|(n, _)| *n == w) {
                Some(&(_, v)) => toks.push(Tok::Num(v)),
                None => toks.push(Tok::Word(w)),
            }
        } else if c == '.' && !chars.get(i + 1).is_some_and(|c| c.is_ascii_alphanumeric()) {
            toks.push(Tok::Num(here as i64));
            i += 1;
        } else {
            toks.push(Tok::Punct(c));
            i += 1;
        }
    }
    toks
}

/// Fold `value (+|-) value` runs into one value, and a `-` with no value
/// before it into the value after it.
fn fold(toks: Vec<Tok>) -> Vec<Tok> {
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = toks[i].clone();
        let sign = match t {
            Tok::Punct(c @ ('+' | '-')) => Some(c),
            _ => None,
        };
        if let (Some(c), Some(Tok::Num(v))) = (sign, toks.get(i + 1)) {
            let v = if c == '-' { -v } else { *v };
            match out.last_mut() {
                Some(Tok::Num(prev)) => *prev += v,
                // a lone `+`, as in `(ix+5)`, stays
                _ if c == '+' => {
                    out.push(t);
                    out.push(Tok::Num(v));
                }
                _ => out.push(Tok::Num(v)),
            }
            i += 2;
            continue;
        }
        out.push(t);
        i += 1;
    }
    out
}

/// The mnemonic and the operand, each normalised; the operand loses its
/// spaces.
pub fn normalise(s: &str, here: u16, symbols: &[(String, i64)]) -> (String, String) {
    let toks = fold(tokenize(s.trim(), here, symbols));
    let mut iter = toks.into_iter().skip_while(|t| *t == Tok::Space);
    let mnemonic = match iter.next() {
        Some(Tok::Word(w)) => w,
        other => format!("{other:?}"),
    };
    let operand = iter
        .filter(|t| *t != Tok::Space)
        .map(|t| match t {
            Tok::Word(w) => w,
            Tok::Num(v) => v.to_string(),
            Tok::Punct(c) => c.to_string(),
            Tok::Space => String::new(),
        })
        .collect();
    (mnemonic, operand)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spelling_goes_and_meaning_stays() {
        let syms = [("offset".to_string(), 0x55)];
        assert_eq!(normalise("bra\t.+0x12", 0x100, &syms), ("bra".into(), "274".into()));
        assert_eq!(normalise("bra 0x0112", 0x100, &syms), ("bra".into(), "274".into()));
        assert_eq!(normalise("LDA  #0x05", 0, &syms), normalise("lda #0x0005", 0, &syms));
        assert_eq!(normalise("neg -16,x", 0, &syms).1, "-16,x");
        assert_eq!(normalise("neg -0x10,x", 0, &syms).1, "-16,x");
        assert_eq!(normalise("adc a,(ix+offset)", 0, &syms).1, "a,(ix+85)");
        assert_eq!(normalise("neg ,--x", 0, &syms).1, ",--x");
    }

    #[test]
    fn cases_carry_their_bytes_and_addresses() {
        let text = "\t.title x\n\tabx\t\t;3a\n; XXX\tcwai #1 ;3c 01\n\tcmps\t*0x0e\t\t;11 9c*0e\n\tfoo = 3\n";
        let cs = cases(text, &|_| false);
        assert_eq!(cs.len(), 2);
        assert_eq!((cs[1].addr, cs[1].bytes.clone(), cs[1].source.as_str()), (1, vec![0x11, 0x9c, 0x0e], "cmps\t*0x0e"));
    }
}
