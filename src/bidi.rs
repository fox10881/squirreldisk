//! Display order for right-to-left text.
//!
//! egui shapes each run of text that uses one font on its own and lays the
//! runs out left to right. Spaces come from Inter and Arabic letters from a
//! system font, so every Arabic word is its own run and a sentence comes out
//! with its words in reverse order. [`visual`] applies the Unicode
//! Bidirectional Algorithm and returns the text in display order, except
//! that each run of Arabic-script characters stays in logical order: the
//! shaper reverses those runs itself, and needs them logical to join letters.

use std::borrow::Cow;
use unicode_bidi::BidiInfo;

/// Strong right-to-left characters (Hebrew, Arabic and related blocks).
fn is_rtl(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{08FF}' | '\u{FB1D}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}' | '\u{10800}'..='\u{10FFF}' | '\u{1E800}'..='\u{1EFFF}')
}

/// Characters egui hands to the shaper as part of a right-to-left run.
fn shaped_rtl(c: char) -> bool {
    is_rtl(c) || matches!(c, '\u{200C}' | '\u{200D}')
}

/// Whether `s` has any right-to-left text, i.e. needs [`visual`].
pub fn has_rtl(s: &str) -> bool {
    !s.is_ascii() && s.chars().any(is_rtl)
}

/// Whether most of the letters in `s` are right-to-left: such text reads
/// from the right, so it is cut and wrapped from the right.
pub fn mostly_rtl(s: &str) -> bool {
    if !has_rtl(s) {
        return false;
    }
    let (rtl, ltr) = s.chars().fold((0, 0), |(r, l), c| {
        if is_rtl(c) {
            (r + 1, l)
        } else if c.is_alphabetic() {
            (r, l + 1)
        } else {
            (r, l)
        }
    });
    rtl >= ltr
}

/// `s` in display order, ready to hand to egui. Text without right-to-left
/// characters is returned as is.
pub fn visual(s: &str) -> Cow<'_, str> {
    if !has_rtl(s) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for (i, line) in s.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&visual_line(line));
    }
    Cow::Owned(out)
}

fn visual_line(line: &str) -> String {
    if !has_rtl(line) {
        return line.to_string();
    }
    let info = BidiInfo::new(line, None);
    let mut order = String::with_capacity(line.len());
    for para in &info.paragraphs {
        let (levels, runs) = info.visual_runs(para, para.range.clone());
        for run in runs {
            let text = &line[run.clone()];
            if levels[run.start].is_rtl() {
                order.extend(text.chars().rev().map(mirror));
            } else {
                order.push_str(text);
            }
        }
    }
    flip_shaped(&order)
}

/// Reverses every run of right-to-left characters. Turns [`visual`]'s output
/// into plain left-to-right character order and back.
pub fn flip_shaped(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if shaped_rtl(chars[i]) {
            let start = i;
            while i < chars.len() && shaped_rtl(chars[i]) {
                i += 1;
            }
            out.extend(chars[start..i].iter().rev());
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Paired punctuation is drawn mirrored in right-to-left text.
fn mirror(c: char) -> char {
    match c {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_words_come_out_in_reading_order() {
        // "chart depth": the first word must end up on the right
        assert_eq!(visual("عمق المخطط"), "المخطط عمق");
        assert_eq!(flip_shaped(&visual("عمق المخطط")), "ططخملا قمع");
    }

    #[test]
    fn left_to_right_text_is_untouched() {
        assert!(matches!(
            visual("Chart depth"),
            Cow::Borrowed("Chart depth")
        ));
        assert_eq!(visual("Photos été"), "Photos été");
    }

    #[test]
    fn embedded_latin_and_numbers_keep_their_order() {
        // "6 rings": the number stays left of the word in display order
        assert_eq!(visual("6 حلقات"), "حلقات 6");
        assert_eq!(visual("فحص SquirrelDisk الآن"), "الآن SquirrelDisk فحص");
        assert_eq!(visual("مجلد (قديم)"), "(قديم) مجلد");
    }

    #[test]
    fn direction_of_mixed_text() {
        assert!(mostly_rtl("عمق المخطط"));
        assert!(!mostly_rtl("Photos مجلد of the summer"));
        assert!(!mostly_rtl("Photos"));
    }
}
