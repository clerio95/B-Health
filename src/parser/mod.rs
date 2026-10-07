//! Lab report parsers. Each supported lab implements [`LabParser`]; adding a
//! lab means adding a module and registering it in [`PARSERS`].

mod common;
mod santa_maria;
mod unimed;

use anyhow::{Result, bail};
use chrono::NaiveDate;

use crate::catalog;
use crate::model::{LabResult, Profile, Sex};
use crate::units::{self, Range};

#[derive(Debug, Clone, Default)]
pub struct ParsedReport {
    pub lab: String,
    pub order_no: Option<String>,
    pub collected_on: Option<NaiveDate>,
    pub patient_name: Option<String>,
    pub birth_date: Option<NaiveDate>,
    pub sex: Option<Sex>,
    pub results: Vec<LabResult>,
}

/// A lab parser extracts results with their raw reference text; explicit
/// ranges (from table rows) may be filled in directly. Ranges left empty are
/// resolved afterwards from the reference text, using the patient's sex and
/// age.
pub trait LabParser: Sync {
    fn lab_name(&self) -> &'static str;
    /// Whether the extracted text looks like a report from this lab.
    fn detect(&self, text: &str) -> bool;
    fn parse(&self, pages: &[String]) -> ParsedReport;
}

static PARSERS: &[&dyn LabParser] = &[&santa_maria::SantaMaria, &unimed::Unimed];

/// Who the report belongs to, used to pick sex- and age-specific ranges.
/// Missing fields fall back to what the report itself says.
#[derive(Debug, Clone, Copy, Default)]
pub struct Patient {
    pub sex: Option<Sex>,
    pub birth_date: Option<NaiveDate>,
}

impl From<Option<&Profile>> for Patient {
    fn from(p: Option<&Profile>) -> Patient {
        Patient {
            sex: p.map(|p| p.sex),
            birth_date: p.map(|p| p.birth_date),
        }
    }
}

/// Extracts text from a PDF and parses it with the matching lab parser.
pub fn parse_pdf(bytes: &[u8], patient: &Patient) -> Result<ParsedReport> {
    let pages = crate::pdf_text::extract_pages(bytes)?;
    parse_pages(&pages, patient)
}

pub fn parse_pages(pages: &[String], patient: &Patient) -> Result<ParsedReport> {
    let all = pages.concat();
    if all.trim().is_empty() {
        bail!(
            "o PDF não contém texto (é uma imagem escaneada?) / the PDF has no text (scanned image?)"
        );
    }
    let Some(parser) = PARSERS.iter().find(|p| p.detect(&all)) else {
        bail!("laboratório não suportado / unsupported lab");
    };
    let mut report = parser.parse(pages);
    report.lab = parser.lab_name().to_string();
    let sex = patient.sex.or(report.sex);
    let age = patient
        .birth_date
        .or(report.birth_date)
        .zip(report.collected_on)
        .and_then(|(birth, on)| on.years_since(birth));
    for r in &mut report.results {
        if r.reference.is_empty() && !r.reference_text.is_empty() {
            let lines: Vec<String> = r.reference_text.lines().map(str::to_string).collect();
            r.reference = units::pick_range(&lines, sex, age);
        }
        canonicalize(r);
    }
    Ok(report)
}

/// Maps a raw result to its catalog biomarker and canonical unit.
fn canonicalize(r: &mut LabResult) {
    r.unit = units::normalize_unit(&r.unit);
    let unit = (!r.unit.is_empty()).then_some(r.unit.as_str());
    let Some(b) = catalog::lookup(&r.original_name, unit) else {
        r.biomarker = format!("custom:{}", catalog::normalize(&r.original_name));
        return;
    };
    r.biomarker = b.key.to_string();
    if r.unit == b.unit || r.unit.is_empty() {
        return;
    }
    let conv = |v: f64| units::convert(b.key, v, &r.unit, b.unit);
    if let Some(value) = r.value
        && let Some(converted) = conv(value)
    {
        r.value = Some(converted);
        r.reference = Range {
            min: r.reference.min.and_then(conv),
            max: r.reference.max.and_then(conv),
        };
        r.unit = b.unit.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_unknown_documents() {
        let p = Patient::default();
        assert!(parse_pages(&["   \n".into()], &p).is_err());
        assert!(parse_pages(&["Laboratório Desconhecido\nGLICOSE 90".into()], &p).is_err());
    }

    #[test]
    fn canonicalize_converts_units() {
        let mut r = LabResult {
            biomarker: String::new(),
            original_name: "Glicose".into(),
            exam: "GLICOSE".into(),
            value: Some(5.0),
            text_value: None,
            unit: "mmol/L".into(),
            reference: Range {
                min: Some(3.9),
                max: Some(5.5),
            },
            reference_text: String::new(),
        };
        canonicalize(&mut r);
        assert_eq!(r.biomarker, "glucose");
        assert_eq!(r.unit, "mg/dL");
        assert!((r.value.unwrap() - 90.08).abs() < 0.01);
    }
}
