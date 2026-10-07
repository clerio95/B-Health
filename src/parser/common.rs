//! Helpers shared by lab parsers.

use chrono::NaiveDate;

use crate::units::{Range, parse_br_number};

pub fn collapse_spaces(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// First `DD/MM/YYYY` date in `s`.
pub fn first_date(s: &str) -> Option<NaiveDate> {
    s.split(|c: char| c.is_whitespace() || c == '-' || c == '(' || c == ')')
        .find_map(|tok| NaiveDate::parse_from_str(tok, "%d/%m/%Y").ok())
}

/// Uppercase words (accents, digits, dots, parentheses and dashes allowed).
pub fn is_upper_words(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.chars().any(|c| c.is_alphabetic())
        && s.chars()
            .all(|c| c.is_uppercase() || c.is_ascii_digit() || " .()-/,".contains(c))
}

/// Whether a token looks like a unit of measure (`mg/dL`, `%`, `fL`, `/mm³`).
pub fn is_unit(tok: &str) -> bool {
    const BARE: &[&str] = &[
        "fl", "pg", "mm³", "µm³", "u", "ui", "mm", "seg", "s", "mui", "ratio",
    ];
    tok == "%"
        || BARE.contains(&tok.to_lowercase().as_str())
        || (tok.contains('/')
            && tok.chars().any(char::is_alphabetic)
            && !tok.ends_with(':')
            && tok.chars().count() <= 16)
}

fn is_range_word(s: &str) -> bool {
    matches!(s, "a" | "A" | "-" | "à" | "até")
}

/// The numeric part of a result line: `value unit [value unit] [min a max
/// unit ...] [anything else]`, where "anything else" is usually the start of
/// a reference column.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Row {
    pub values: Vec<(f64, String)>,
    pub ranges: Vec<(Range, String)>,
    pub rest: String,
}

pub fn parse_row(text: &str) -> Option<Row> {
    // Tokens with their byte offsets, so the leftover text keeps its spacing.
    let tokens: Vec<(usize, &str)> = text
        .split_whitespace()
        .map(|t| (t.as_ptr() as usize - text.as_ptr() as usize, t))
        .collect();
    let tok = |i: usize| tokens.get(i).map(|t| t.1);
    let num = |i: usize| tok(i).and_then(parse_br_number);

    let mut i = 0;
    let mut row = Row::default();
    while let (Some(v), Some(unit)) = (num(i), tok(i + 1)) {
        if !is_unit(unit) {
            break;
        }
        row.values.push((v, unit.to_string()));
        i += 2;
    }
    if row.values.is_empty() {
        return None;
    }
    loop {
        let start = if matches!(tok(i), Some("De" | "de" | "DE")) {
            i + 1
        } else {
            i
        };
        let (Some(min), Some(true), Some(max)) = (
            num(start),
            tok(start + 1).map(is_range_word),
            num(start + 2),
        ) else {
            break;
        };
        let unit = tok(start + 3);
        let unit = match unit {
            None => None,
            Some(u) if is_unit(u) => Some(u),
            Some(u) if parse_br_number(u).is_some() => None,
            // `2 a 7 anos: ...` is an age band, not a range.
            Some(_) => break,
        };
        let text = [tok(start), tok(start + 1), tok(start + 2), unit]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        row.ranges.push((
            Range {
                min: Some(min),
                max: Some(max),
            },
            text,
        ));
        i = start + if unit.is_some() { 4 } else { 3 };
    }
    row.rest = tokens
        .get(i)
        .map(|(offset, _)| text[*offset..].trim().to_string())
        .unwrap_or_default();
    Some(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: f64, max: f64) -> Range {
        Range {
            min: Some(min),
            max: Some(max),
        }
    }

    #[test]
    fn parses_rows() {
        let r = parse_row("5,49 milhões/mm³      4,50 a 5,50 milhões/mm³").unwrap();
        assert_eq!(r.values, vec![(5.49, "milhões/mm³".into())]);
        assert_eq!(r.ranges[0].0, range(4.5, 5.5));
        assert!(r.rest.is_empty());

        let r = parse_row("1,0 %   50 /mm³   0,0 a 5,0 %   0 a 600 /mm³").unwrap();
        assert_eq!(r.values.len(), 2);
        assert_eq!(r.ranges[1].0, range(0.0, 600.0));

        let r = parse_row("29,9 pg     De 27,0 a 32,0 pg").unwrap();
        assert_eq!(r.ranges[0].0, range(27.0, 32.0));

        let r = parse_row("2.49 µUI/mL     0 a 3 dias......: 1.10 a 15.70 µUI/mL").unwrap();
        assert!(r.ranges.is_empty());
        assert_eq!(r.rest, "0 a 3 dias......: 1.10 a 15.70 µUI/mL");

        let r = parse_row("4.78 %     Normal: Inferior a 5,7%").unwrap();
        assert_eq!(r.values, vec![(4.78, "%".into())]);
        assert_eq!(r.rest, "Normal: Inferior a 5,7%");

        assert!(parse_row("2018 62(4): 386-91").is_none());
        assert!(parse_row("7 a 13 anos: 2,5 a 10,4 ng/dL").is_none());
    }

    #[test]
    fn recognizes_units() {
        for u in [
            "mg/dL",
            "%",
            "fL",
            "/mm³",
            "µUI/mL",
            "milhões/mm³",
            "µm³",
            "U/L",
        ] {
            assert!(is_unit(u), "{u}");
        }
        for u in ["anos:", "a", "62(4):", "Normal:"] {
            assert!(!is_unit(u), "{u}");
        }
    }
}
