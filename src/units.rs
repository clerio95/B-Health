//! Brazilian number formats, unit normalization, conversions and parsing of
//! reference-range text.

use crate::model::Sex;

/// Parses a number written in Brazilian format: `,` is the decimal separator
/// and `.` groups thousands (`1.080` = 1080, `5,26` = 5.26).
pub fn parse_br_number(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('%');
    if s.is_empty()
        || !s
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'))
        || !s.chars().any(|c| c.is_ascii_digit())
    {
        return None;
    }
    let cleaned = if s.contains(',') {
        s.replace('.', "").replace(',', ".")
    } else if is_thousands_grouped(s) {
        s.replace('.', "")
    } else {
        s.to_string()
    };
    cleaned.parse().ok()
}

/// `1.080`, `150.000`, `1.234.567` → true; `1.5`, `12.25` → false.
fn is_thousands_grouped(s: &str) -> bool {
    let s = s.trim_start_matches(['-', '+']);
    let mut parts = s.split('.');
    let first = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    !rest.is_empty()
        && (1..=3).contains(&first.len())
        && !first.starts_with('0')
        && rest
            .iter()
            .all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
}

/// Formats a number Brazilian-style or English-style.
pub fn format_number(v: f64, decimal_comma: bool) -> String {
    let decimals = if v.abs() >= 1000.0 || v.fract() == 0.0 {
        0
    } else if v.abs() >= 100.0 {
        1
    } else {
        2
    };
    let mut s = format!("{v:.decimals$}");
    if decimals > 0 {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    let (int, frac) = match s.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (s.clone(), None),
    };
    let (sign, digits) = match int.strip_prefix('-') {
        Some(d) => ("-", d),
        None => ("", int.as_str()),
    };
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(if decimal_comma { '.' } else { ',' });
        }
        grouped.push(c);
    }
    match frac {
        Some(f) => format!(
            "{sign}{grouped}{}{f}",
            if decimal_comma { ',' } else { '.' }
        ),
        None => format!("{sign}{grouped}"),
    }
}

/// Canonical spelling for units as printed by labs.
pub fn normalize_unit(unit: &str) -> String {
    let u = unit.trim();
    let lower = u.to_lowercase().replace(' ', "");
    let canonical = match lower.as_str() {
        "mm³" | "/mm³" | "mm3" | "/mm3" | "/µl" | "/ul" | "células/mm³" | "cel/mm³" => "/mm³",
        "milhões/mm³" | "milhoes/mm³" | "milhões/mm3" | "x10^6/µl" | "10^6/µl" | "milhões/µl" => {
            "milhões/mm³"
        }
        "microui/ml" | "µui/ml" | "uui/ml" | "µiu/ml" | "uiu/ml" | "mui/l" | "miu/l" => "µUI/mL",
        "mcg/dl" | "µg/dl" | "ug/dl" => "µg/dL",
        "meq/l" | "mmol/l(na/k)" => "mEq/L",
        "mg/dl" => "mg/dL",
        "g/dl" => "g/dL",
        "ng/ml" => "ng/mL",
        "ng/dl" => "ng/dL",
        "pg/ml" => "pg/mL",
        "mg/l" => "mg/L",
        "u/l" | "ui/l" => "U/L",
        "fl" | "µm³" | "um³" => "fL",
        "nmol/l" => "nmol/L",
        "pg" => "pg",
        "%" => "%",
        "mmol/l" => "mmol/L",
        "µmol/l" | "umol/l" | "micromol/l" => "µmol/L",
        "mm/h" | "mm/1ªhora" | "mm/1ahora" => "mm/h",
        _ => return u.to_string(),
    };
    canonical.to_string()
}

/// Converts `value` in `unit` to the biomarker's canonical unit, when a
/// conversion is known. Returns `None` if units are incompatible/unknown.
pub fn convert(key: &str, value: f64, unit: &str, target: &str) -> Option<f64> {
    let unit = normalize_unit(unit);
    if unit == target {
        return Some(value);
    }
    let factor = match (key, unit.as_str(), target) {
        ("glucose" | "eag", "mmol/L", "mg/dL") => 18.016,
        ("total_cholesterol" | "hdl" | "ldl" | "vldl", "mmol/L", "mg/dL") => 38.67,
        ("triglycerides", "mmol/L", "mg/dL") => 88.57,
        // g/dL and % express the same MCHC value.
        ("mchc", "g/dL", "%") => 1.0,
        ("creatinine", "µmol/L", "mg/dL") => 1.0 / 88.4,
        ("urea", "mmol/L", "mg/dL") => 6.006,
        ("uric_acid", "µmol/L", "mg/dL") => 1.0 / 59.48,
        ("calcium", "mmol/L", "mg/dL") => 4.008,
        ("magnesium", "mmol/L", "mg/dL") => 2.431,
        ("sodium" | "potassium", "mmol/L", "mEq/L") => 1.0,
        ("vitamin_d", "nmol/L", "ng/mL") => 1.0 / 2.496,
        ("vitamin_b12", "pmol/L", "pg/mL") => 1.355,
        (_, "x10^3/µL" | "10^3/µL" | "mil/mm³", "/mm³") => 1000.0,
        _ => return None,
    };
    Some(value * factor)
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Range {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl Range {
    pub fn is_empty(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }
}

/// Words that introduce an upper limit / a lower limit (normalized).
const UPPER_WORDS: &[&str] = &[
    "INFERIOR A",
    "MENOR QUE",
    "ABAIXO DE",
    "ATE",
    "MENOR OU IGUAL A",
];
const LOWER_WORDS: &[&str] = &[
    "SUPERIOR A",
    "MAIOR QUE",
    "ACIMA DE",
    "ACIMA",
    "MAIOR OU IGUAL A",
];

/// Numbers found in `s`, as (char start, char end, value).
fn numbers_in(s: &str) -> Vec<(usize, usize, f64)> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut start = None;
    for i in 0..=chars.len() {
        let is_num = chars.get(i).is_some_and(|&c| {
            c.is_ascii_digit()
                || (matches!(c, ',' | '.')
                    && start.is_some()
                    && chars.get(i + 1).is_some_and(|n| n.is_ascii_digit()))
        });
        match (is_num, start) {
            (true, None) => start = Some(i),
            (false, Some(st)) => {
                let text: String = chars[st..i].iter().collect();
                if let Some(v) = parse_br_number(&text) {
                    out.push((st, i, v));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

fn has_word(normalized: &str, word: &str) -> bool {
    normalized == word
        || normalized.ends_with(&format!(" {word}"))
        || normalized.starts_with(&format!("{word} "))
        || normalized.contains(&format!(" {word} "))
}

/// Parses a single reference-range expression such as `DE 60 a 99 mg/dL`,
/// `Até 40 U/L`, `> 60 mg/dL`, `Superior a 3,37 ng/mL`, `8,5 a 10,5`.
pub fn parse_range_expr(text: &str) -> Range {
    let nums = numbers_in(text);
    let Some(&(first_start, first_end, first)) = nums.first() else {
        return Range::default();
    };
    let slice =
        |from: usize, to: usize| -> String { text.chars().skip(from).take(to - from).collect() };

    // "X a Y", "X A Y", "X - Y", "entre X e Y", "5,7% a 6,4%"
    if let Some(&(second_start, _, second)) = nums.get(1) {
        let between = slice(first_end, second_start);
        let norm = crate::catalog::normalize(&between);
        if matches!(norm.as_str(), "A" | "E" | "") && first <= second {
            return Range {
                min: Some(first),
                max: Some(second),
            };
        }
    }

    let prefix = slice(0, first_start);
    let symbols: String = prefix.chars().filter(|c| matches!(c, '<' | '>')).collect();
    let words = crate::catalog::normalize(&prefix);
    let upper = symbols.ends_with('<') || UPPER_WORDS.iter().any(|w| has_word(&words, w));
    let lower = symbols.ends_with('>') || LOWER_WORDS.iter().any(|w| has_word(&words, w));
    // Check upper first: "menor ou igual a" must not be read as a lower bound.
    if upper {
        Range {
            min: None,
            max: Some(first),
        }
    } else if lower {
        Range {
            min: Some(first),
            max: None,
        }
    } else {
        Range::default()
    }
}

/// Labels that identify the "normal"/healthy line inside multi-line reference
/// blocks, from most to least preferred.
const PREFERRED_LABELS: &[&str] = &[
    "NORMAL",
    "OTIMO",
    "DESEJAVEL",
    "ADEQUADO",
    "ADULTOS",
    "ADULTO",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Group {
    Sex(Sex),
    Pregnancy,
}

/// Recognizes `Masculino`, `Homens`, `Mulheres adultas`, `Gestantes`...
fn group_of(normalized: &str) -> Option<Group> {
    match normalized.split(' ').next().unwrap_or("") {
        "MASCULINO" | "HOMEM" | "HOMENS" => Some(Group::Sex(Sex::Male)),
        "FEMININO" | "MULHER" | "MULHERES" => Some(Group::Sex(Sex::Female)),
        "GESTANTE" | "GESTANTES" | "GRAVIDAS" | "GESTACAO" => Some(Group::Pregnancy),
        _ => None,
    }
}

/// Age span (in years) described by a label such as `15 a 60 anos`,
/// `> 19 anos`, `Superior a 80 anos`, `Crianças até 5 anos` or `0 a 1 mês`.
fn age_span(raw_label: &str) -> Option<(f64, f64)> {
    let norm = crate::catalog::normalize(raw_label);
    let year = norm.find("ANO");
    let infant = ["MES", "DIA"].iter().filter_map(|w| norm.find(w)).min();
    if let Some(i) = infant
        && year.is_none_or(|y| i < y)
    {
        return Some((0.0, 1.99));
    }
    year?;
    let nums: Vec<f64> = numbers_in(raw_label).into_iter().map(|n| n.2).collect();
    let first = *nums.first()?;
    let symbols: String = raw_label
        .chars()
        .filter(|c| matches!(c, '<' | '>'))
        .collect();
    let words_before: String = {
        let first_digit = raw_label.find(|c: char| c.is_ascii_digit()).unwrap_or(0);
        crate::catalog::normalize(&raw_label[..first_digit])
    };
    if symbols.starts_with('>') || LOWER_WORDS.iter().any(|w| has_word(&words_before, w)) {
        Some((first, f64::INFINITY))
    } else if symbols.starts_with('<') || UPPER_WORDS.iter().any(|w| has_word(&words_before, w)) {
        Some((0.0, first))
    } else {
        Some((first, *nums.last()?))
    }
}

/// Removes `( ... )` asides such as `(Carência: abaixo de 30,0 ug/dL)`.
fn strip_parentheses(line: &str) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    for c in line.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Picks the reference range that applies to this person from a (possibly
/// multi-line) reference block.
///
/// Lines for the other sex, for pregnancy or for other age groups are
/// skipped; then lines matching the person's age, lines explicitly for their
/// sex, and lines labeled normal/optimal/desirable/adult win, in that order.
/// Otherwise the first line with a parseable range is used.
pub fn pick_range(lines: &[String], sex: Option<Sex>, age: Option<u32>) -> Range {
    struct Candidate {
        range: Range,
        age_matched: bool,
        sex_explicit: bool,
        preferred: usize,
    }
    let mut group: Option<Group> = None;
    let mut candidates = Vec::new();
    let mut all_age_labeled = true;
    let mut raw = Vec::new();
    for line in lines {
        let line = strip_parentheses(line);
        let (label, expr) = split_label(&line);
        let norm = crate::catalog::normalize(&label);
        let range = parse_range_expr(&expr);
        if range.is_empty() {
            if let Some(g) = group_of(&crate::catalog::normalize(&line)) {
                group = Some(g);
            }
            continue;
        }
        let own = group_of(&norm).or(group);
        let span = age_span(&label);
        all_age_labeled &= span.is_some();
        raw.push((range, own, span, norm));
    }
    for (range, own, span, norm) in raw {
        let sex_explicit = match own {
            Some(Group::Pregnancy) => continue,
            Some(Group::Sex(s)) if sex.is_some_and(|x| x != s) => continue,
            Some(Group::Sex(_)) => sex.is_some(),
            None => false,
        };
        let age_matched = match (span, age) {
            (Some((lo, hi)), Some(a)) => {
                let a = a as f64;
                if a < lo || a > hi {
                    continue;
                }
                true
            }
            (Some(_), None) if !all_age_labeled => continue,
            _ => false,
        };
        let preferred = PREFERRED_LABELS
            .iter()
            .position(|p| norm.split(' ').any(|w| w == *p))
            .unwrap_or(PREFERRED_LABELS.len());
        candidates.push(Candidate {
            range,
            age_matched,
            sex_explicit,
            preferred,
        });
    }
    candidates
        .iter()
        .enumerate()
        .min_by_key(|(i, c)| (!c.age_matched, !c.sex_explicit, c.preferred, *i))
        .map(|(_, c)| c.range)
        .unwrap_or_default()
}

/// Splits `Label......: expression` into (label, expression). Without a
/// colon, the label is the leading text before the first number or `<`/`>`
/// (`Homens > 55 mg/dL` → `Homens`) and the expression is the whole line, so
/// words like `menor que` still qualify the number.
fn split_label(line: &str) -> (String, String) {
    let line = line.trim();
    match line.rfind(':') {
        Some(i) if line[i + 1..].chars().any(|c| c.is_ascii_digit()) => {
            (line[..i].to_string(), line[i + 1..].to_string())
        }
        _ => {
            let start = line
                .find(|c: char| c.is_ascii_digit() || matches!(c, '<' | '>'))
                .unwrap_or(line.len());
            (line[..start].to_string(), line.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_brazilian_numbers() {
        assert_eq!(parse_br_number("5,26"), Some(5.26));
        assert_eq!(parse_br_number("1.080"), Some(1080.0));
        assert_eq!(parse_br_number("150.000"), Some(150_000.0));
        assert_eq!(parse_br_number("1.234,5"), Some(1234.5));
        assert_eq!(parse_br_number("77"), Some(77.0));
        assert_eq!(parse_br_number("4,20%"), Some(4.2));
        assert_eq!(parse_br_number("mg/dL"), None);
        assert_eq!(parse_br_number("a"), None);
        // Dot decimals (some labs) still work when not a thousands group.
        assert_eq!(parse_br_number("43.4"), Some(43.4));
        assert_eq!(parse_br_number("1.24"), Some(1.24));
        assert_eq!(parse_br_number("0.930"), Some(0.93));
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(format_number(5.26, true), "5,26");
        assert_eq!(format_number(198000.0, true), "198.000");
        assert_eq!(format_number(198000.0, false), "198,000");
        assert_eq!(format_number(141.7, true), "141,7");
        assert_eq!(format_number(4.0, true), "4");
        assert_eq!(format_number(-12.5, false), "-12.5");
    }

    #[test]
    fn normalizes_units() {
        assert_eq!(normalize_unit("mm³"), "/mm³");
        assert_eq!(normalize_unit("microUI/mL"), "µUI/mL");
        assert_eq!(normalize_unit("mEq/L"), "mEq/L");
        assert_eq!(normalize_unit("weird"), "weird");
    }

    #[test]
    fn converts_units() {
        let v = convert("glucose", 5.0, "mmol/L", "mg/dL").unwrap();
        assert!((v - 90.08).abs() < 0.01);
        assert_eq!(convert("glucose", 90.0, "mg/dL", "mg/dL"), Some(90.0));
        assert_eq!(convert("glucose", 90.0, "g/L", "mg/dL"), None);
    }

    fn r(min: Option<f64>, max: Option<f64>) -> Range {
        Range { min, max }
    }

    #[test]
    fn parses_range_expressions() {
        assert_eq!(
            parse_range_expr("DE 60 a 99 mg/dL"),
            r(Some(60.0), Some(99.0))
        );
        assert_eq!(parse_range_expr("1,6 A 2,5 mg/dL"), r(Some(1.6), Some(2.5)));
        assert_eq!(
            parse_range_expr("4.500 a 11.000 mm³"),
            r(Some(4500.0), Some(11000.0))
        );
        assert_eq!(parse_range_expr("Até 40 U/L"), r(None, Some(40.0)));
        assert_eq!(parse_range_expr("ATÉ 6,0 mg/L"), r(None, Some(6.0)));
        assert_eq!(parse_range_expr("> 60 mg/dL"), r(Some(60.0), None));
        assert_eq!(parse_range_expr("< 200 mg/dL"), r(None, Some(200.0)));
        assert_eq!(
            parse_range_expr("Superior a 3,37 ng/mL"),
            r(Some(3.37), None)
        );
        assert_eq!(parse_range_expr("menor que 5,7%"), r(None, Some(5.7)));
        assert_eq!(
            parse_range_expr("ENTRE 20,0 A 60,0 ng/mL"),
            r(Some(20.0), Some(60.0))
        );
        assert_eq!(parse_range_expr("Não reagente"), r(None, None));
    }

    #[test]
    fn picks_preferred_line() {
        let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tgo = lines(&[
            "Crianças até 5 anos: Até 50 U/L",
            "Adultos............: Até 40 U/L",
        ]);
        assert_eq!(pick_range(&tgo, None, Some(30)), r(None, Some(40.0)));

        let vitd = lines(&[
            "DEFICIENTE.....................: INFERIOR A 20,0 ng/mL",
            "ADEQUADO PARA A POPULAÇÃO GERAL: ENTRE 20,0 A 60,0 ng/mL",
            "IDEAL (GRUPOS DE RISCO)........: DE 30,0 A 60,0 ng/mL",
        ]);
        assert_eq!(pick_range(&vitd, None, Some(30)), r(Some(20.0), Some(60.0)));

        let ldl = lines(&[
            "Ótimo.....:      < 100 mg/dL",
            "Desejável.: 100 a 129 mg/dL",
            "Alto......: 160 a 189 mg/dL",
        ]);
        assert_eq!(pick_range(&ldl, None, Some(30)), r(None, Some(100.0)));

        let prl = lines(&["HOMEM.: DE 3,4 A 19,4 ng/mL", "MULHER: DE 4,8 A 23,3 ng/mL"]);
        assert_eq!(
            pick_range(&prl, Some(Sex::Male), Some(30)),
            r(Some(3.4), Some(19.4))
        );
        assert_eq!(
            pick_range(&prl, Some(Sex::Female), Some(30)),
            r(Some(4.8), Some(23.3))
        );

        let glucose = lines(&[
            "normal........................: DE  60 a  99 mg/dL",
            "glicose de jejum alterada.....: DE 100 a 125 mg/dL",
        ]);
        assert_eq!(
            pick_range(&glucose, None, Some(30)),
            r(Some(60.0), Some(99.0))
        );
    }

    #[test]
    fn picks_by_age_sex_and_group() {
        let lines = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let tsh = lines(&[
            "0 a 3 dias.................: 1.10 a 15.70 µUI/mL",
            "2meses 14dias a 1ano 3meses: 0.40 a 6.00 µUI/mL",
            "1 ano e 3 meses a 6 anos...: 0.40 a 5.20 µUI/mL",
            "15 a 60 anos...............: 0.40 a 4.30 µUI/mL",
            "60 a 80 anos...............: 0.40 a 5.80 µUI/mL",
            "Superior a 80 anos.........: 0.40 a 6.70 µUI/mL",
            "Gestantes:",
            "Primeiro Trimestre.........: 0.10 a 3.60 µUI/mL",
        ]);
        assert_eq!(
            pick_range(&tsh, Some(Sex::Male), Some(30)),
            r(Some(0.4), Some(4.3))
        );
        assert_eq!(
            pick_range(&tsh, Some(Sex::Male), Some(85)),
            r(Some(0.4), Some(6.7))
        );
        assert_eq!(
            pick_range(&tsh, Some(Sex::Male), Some(4)),
            r(Some(0.4), Some(5.2))
        );

        let b12 = lines(&[
            "0 a 1 mês...: 187-1866 pg/mL",
            "12 a 19 anos: 270-1132 pg/mL",
            "> 19 anos...: 245-985  pg/mL",
        ]);
        assert_eq!(
            pick_range(&b12, None, Some(30)),
            r(Some(245.0), Some(985.0))
        );

        let testosterone = lines(&[
            "Não existem valores de referências pré-definidos",
            "para crianças menores de 2 anos",
            "Feminino:",
            "20 a 50 anos......: 8.4 a 48.1 ng/dL",
            "Masculino:",
            "18 a 20 anos......: 188 a 882 ng/dL",
            "20 a 50 anos......: 249 a 836 ng/dL",
        ]);
        assert_eq!(
            pick_range(&testosterone, Some(Sex::Male), Some(30)),
            r(Some(249.0), Some(836.0))
        );
        assert_eq!(
            pick_range(&testosterone, Some(Sex::Female), Some(30)),
            r(Some(8.4), Some(48.1))
        );

        let hdl = lines(&[
            "Sem risco    Risco moderado   Risco elevado",
            "Mulheres > 65 mg/dL    45-65 mg/dL       < 45 mg/dL",
            "Homens   > 55 mg/dL    35-55 mg/dL       < 35 mg/dL",
            "< 40 mg/dL: Colesterol HDL baixo (principal fator de",
        ]);
        assert_eq!(
            pick_range(&hdl, Some(Sex::Male), Some(30)),
            r(Some(55.0), None)
        );

        let zinc = lines(&[
            "70,0 a 120,0 ug/dL (Carência:abaixo de 30,0 ug/dL)",
            "Até 170,0 ug/dL (Exposição ocupacional)",
        ]);
        assert_eq!(
            pick_range(&zinc, None, Some(30)),
            r(Some(70.0), Some(120.0))
        );

        let vitd = lines(&[
            "Desejável para população normal: Acima 20.0 ng/mL",
            "Desejável para grupos de risco.: De 30.0 a 60.0 ng/mL",
        ]);
        assert_eq!(pick_range(&vitd, None, Some(30)), r(Some(20.0), None));
    }
}
