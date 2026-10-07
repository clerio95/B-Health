use chrono::NaiveDate;

use crate::units::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sex {
    Male,
    Female,
}

impl Sex {
    pub fn code(self) -> &'static str {
        match self {
            Sex::Male => "M",
            Sex::Female => "F",
        }
    }

    pub fn from_code(code: &str) -> Option<Sex> {
        match code.trim() {
            "M" | "m" => Some(Sex::Male),
            "F" | "f" => Some(Sex::Female),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Profile {
    pub name: String,
    pub birth_date: NaiveDate,
    pub sex: Sex,
    pub height_cm: f64,
}

impl Profile {
    /// Age in whole years on `date`.
    pub fn age_on(&self, date: NaiveDate) -> u32 {
        date.years_since(self.birth_date).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub struct WeightEntry {
    pub date: NaiveDate,
    pub kg: f64,
}

/// One imported lab report (one PDF).
#[derive(Debug, Clone)]
pub struct Report {
    pub id: i64,
    pub lab: String,
    pub order_no: Option<String>,
    pub collected_on: NaiveDate,
    pub file_name: String,
}

/// One measured value inside a report.
#[derive(Debug, Clone, PartialEq)]
pub struct LabResult {
    /// Catalog key, or `custom:<NORMALIZED NAME>` for unknown biomarkers.
    pub biomarker: String,
    pub original_name: String,
    pub exam: String,
    pub value: Option<f64>,
    pub text_value: Option<String>,
    pub unit: String,
    pub reference: Range,
    pub reference_text: String,
}

/// A stored result joined with its report date and lab.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub report_id: i64,
    pub date: NaiveDate,
    pub lab: String,
    pub result: LabResult,
}
