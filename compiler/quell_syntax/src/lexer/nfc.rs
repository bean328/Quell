//! Unicode Normalization Form C, used to check identifiers (spec/lexical.md, section 5.1).
//!
//! This is the standard algorithm from Unicode Standard Annex #15: decompose every character
//! canonically, put combining marks in canonical order, then compose again. The data comes from
//! `nfc_tables.rs`; Hangul syllables are decomposed and composed by arithmetic.

use super::nfc_tables::{COMBINING_CLASSES, COMPOSITIONS, DECOMPOSITIONS};

const HANGUL_S_BASE: u32 = 0xAC00;
const HANGUL_L_BASE: u32 = 0x1100;
const HANGUL_V_BASE: u32 = 0x1161;
const HANGUL_T_BASE: u32 = 0x11A7;
const HANGUL_L_COUNT: u32 = 19;
const HANGUL_V_COUNT: u32 = 21;
const HANGUL_T_COUNT: u32 = 28;
const HANGUL_N_COUNT: u32 = HANGUL_V_COUNT * HANGUL_T_COUNT;
const HANGUL_S_COUNT: u32 = HANGUL_L_COUNT * HANGUL_N_COUNT;

/// Returns the NFC form of `text`, or `None` if `text` is already in NFC.
pub(crate) fn nfc_if_different(text: &str) -> Option<String> {
    // Every character below U+0300 is unchanged by NFC and never combines with what comes
    // before it, so most names are settled here without any table lookups.
    if text.chars().all(|c| (c as u32) < 0x300) {
        return None;
    }
    let normalized = nfc(text);
    if normalized == text { None } else { Some(normalized) }
}

/// The NFC form of `text`.
pub(crate) fn nfc(text: &str) -> String {
    let mut chars = Vec::with_capacity(text.len());
    for c in text.chars() {
        decompose(c as u32, &mut chars);
    }
    reorder(&mut chars);
    compose(&mut chars);
    chars.into_iter().filter_map(char::from_u32).collect()
}

fn combining_class(c: u32) -> u8 {
    let index = COMBINING_CLASSES.partition_point(|&(_, last, _)| last < c);
    match COMBINING_CLASSES.get(index) {
        Some(&(first, _, class)) if first <= c => class,
        _ => 0,
    }
}

fn decompose(c: u32, out: &mut Vec<u32>) {
    if (HANGUL_S_BASE..HANGUL_S_BASE + HANGUL_S_COUNT).contains(&c) {
        let index = c - HANGUL_S_BASE;
        out.push(HANGUL_L_BASE + index / HANGUL_N_COUNT);
        out.push(HANGUL_V_BASE + (index % HANGUL_N_COUNT) / HANGUL_T_COUNT);
        let t = index % HANGUL_T_COUNT;
        if t != 0 {
            out.push(HANGUL_T_BASE + t);
        }
        return;
    }
    match DECOMPOSITIONS.binary_search_by_key(&c, |&(from, _, _)| from) {
        Ok(index) => {
            let (_, first, second) = DECOMPOSITIONS[index];
            // Canonical decompositions are at most a few levels deep, so this recursion is
            // bounded by the Unicode data, not by the input.
            decompose(first, out);
            if second != 0 {
                decompose(second, out);
            }
        }
        Err(_) => out.push(c),
    }
}

/// Sorts each run of combining marks by combining class, keeping equal classes in order.
fn reorder(chars: &mut [u32]) {
    let mut start = 0;
    while start < chars.len() {
        if combining_class(chars[start]) == 0 {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < chars.len() && combining_class(chars[end]) != 0 {
            end += 1;
        }
        chars[start..end].sort_by_key(|&c| combining_class(c));
        start = end;
    }
}

fn compose_pair(first: u32, second: u32) -> Option<u32> {
    let l_range = HANGUL_L_BASE..HANGUL_L_BASE + HANGUL_L_COUNT;
    let v_range = HANGUL_V_BASE..HANGUL_V_BASE + HANGUL_V_COUNT;
    if l_range.contains(&first) && v_range.contains(&second) {
        let l = first - HANGUL_L_BASE;
        let v = second - HANGUL_V_BASE;
        return Some(HANGUL_S_BASE + (l * HANGUL_V_COUNT + v) * HANGUL_T_COUNT);
    }
    let s_range = HANGUL_S_BASE..HANGUL_S_BASE + HANGUL_S_COUNT;
    let t_range = HANGUL_T_BASE + 1..HANGUL_T_BASE + HANGUL_T_COUNT;
    if s_range.contains(&first)
        && (first - HANGUL_S_BASE) % HANGUL_T_COUNT == 0
        && t_range.contains(&second)
    {
        return Some(first + (second - HANGUL_T_BASE));
    }
    COMPOSITIONS
        .binary_search_by(|&(a, b, _)| (a, b).cmp(&(first, second)))
        .ok()
        .map(|index| COMPOSITIONS[index].2)
}

fn compose(chars: &mut Vec<u32>) {
    let mut out: Vec<u32> = Vec::with_capacity(chars.len());
    let mut starter: Option<usize> = None;
    let mut last_class = 0u8;
    for &c in chars.iter() {
        let class = combining_class(c);
        if let Some(s) = starter {
            let adjacent = out.len() == s + 1;
            if adjacent || (last_class != 0 && last_class < class) {
                if let Some(composed) = compose_pair(out[s], c) {
                    out[s] = composed;
                    continue;
                }
            }
        }
        if class == 0 {
            starter = Some(out.len());
        }
        last_class = class;
        out.push(c);
    }
    *chars = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_and_composed_names_are_unchanged() {
        assert_eq!(nfc_if_different("score"), None);
        assert_eq!(nfc_if_different("caf\u{E9}"), None);
        assert_eq!(nfc_if_different("\u{D55C}\u{AE00}"), None);
        assert_eq!(nfc_if_different("\u{5929}\u{6C14}"), None);
    }

    #[test]
    fn combining_marks_are_composed() {
        assert_eq!(nfc_if_different("cafe\u{301}").as_deref(), Some("caf\u{E9}"));
        // u, combining diaeresis, combining macron gives one character.
        assert_eq!(nfc("u\u{308}\u{304}"), "\u{1D6}");
        // Marks in the wrong order are put in canonical order first.
        assert_eq!(nfc("a\u{301}\u{323}"), "\u{1EA1}\u{301}");
    }

    #[test]
    fn singletons_and_hangul() {
        // The Kelvin sign and the Angstrom sign are replaced by ordinary letters.
        assert_eq!(nfc("\u{212A}"), "K");
        assert_eq!(nfc("\u{212B}"), "\u{C5}");
        // Hangul jamo compose into a syllable.
        assert_eq!(nfc("\u{1112}\u{1161}\u{11AB}"), "\u{D55C}");
    }
}
