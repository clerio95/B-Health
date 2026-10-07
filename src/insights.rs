//! Turns stored measurements into per-biomarker series, range statuses,
//! notable changes and derived metrics. Everything here is deterministic and
//! local; it informs, it does not diagnose.

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDate;

use crate::catalog::{self, Biomarker, Panel};
use crate::i18n::{Key, Lang};
use crate::model::{Measurement, Profile, Sex, WeightEntry};
use crate::units::Range;

/// Relative change between consecutive tests that is flagged as notable.
pub const BIG_CHANGE_PCT: f64 = 25.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Low,
    Normal,
    High,
    Unknown,
}

impl Status {
    pub fn of(value: Option<f64>, range: &Range) -> Status {
        let Some(v) = value else {
            return Status::Unknown;
        };
        if range.is_empty() {
            return Status::Unknown;
        }
        if range.min.is_some_and(|m| v < m) {
            Status::Low
        } else if range.max.is_some_and(|m| v > m) {
            Status::High
        } else {
            Status::Normal
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        lang.t(match self {
            Status::Low => Key::Low,
            Status::Normal => Key::Normal,
            Status::High => Key::High,
            Status::Unknown => Key::Unknown,
        })
    }

    /// Like [`Status::label`], but against a goal range (see
    /// [`is_goal_range`]) misses read "below/above desirable".
    pub fn goal_label(self, goal: bool, lang: Lang) -> &'static str {
        match self {
            Status::Low if goal => lang.t(Key::BelowGoal),
            Status::High if goal => lang.t(Key::AboveGoal),
            _ => self.label(lang),
        }
    }

    pub fn is_out(self) -> bool {
        matches!(self, Status::Low | Status::High)
    }
}

/// Whether a lab reference describes a goal ("desirable", "optimal", risk
/// tiers) rather than a normal interval. Missing such a goal is milder than
/// being abnormal: HDL 56 against "Desejável: > 60" is not "low" when the
/// lab's own "Baixo" is < 40.
pub fn is_goal_range(reference_text: &str) -> bool {
    const GOAL: &[&str] = &[
        "DESEJAVEL",
        "OTIMO",
        "IDEAL",
        "LIMITROFE",
        "SEM RISCO",
        "RISCO BAIXO",
    ];
    const NORMAL: &[&str] = &["NORMAL", "ADEQUADO"];
    let text = format!(" {} ", catalog::normalize(reference_text));
    let has = |w: &&str| text.contains(&format!(" {w} "));
    GOAL.iter().any(has) && !NORMAL.iter().any(has)
}

/// All measurements of one biomarker, oldest first.
#[derive(Debug, Clone)]
pub struct Series {
    pub key: String,
    pub def: Option<&'static Biomarker>,
    pub original_name: String,
    pub points: Vec<Measurement>,
    pub target: Option<Range>,
}

impl Series {
    pub fn name(&self, lang: Lang) -> String {
        match self.def {
            Some(b) => b.name(lang).to_string(),
            None => title_case(&self.original_name),
        }
    }

    pub fn panel(&self) -> Panel {
        self.def.map_or(Panel::Other, |b| b.panel)
    }

    pub fn latest(&self) -> &Measurement {
        self.points.last().expect("series is never empty")
    }

    pub fn previous(&self) -> Option<&Measurement> {
        self.points.len().checked_sub(2).map(|i| &self.points[i])
    }

    pub fn status(&self) -> Status {
        let r = &self.latest().result;
        Status::of(r.value, &r.reference)
    }

    /// Whether the latest lab reference is a goal rather than a normal range.
    pub fn goal_range(&self) -> bool {
        is_goal_range(&self.latest().result.reference_text)
    }

    pub fn target_status(&self) -> Option<Status> {
        self.target
            .map(|t| Status::of(self.latest().result.value, &t))
    }

    /// Change from the previous test, as (absolute, percent).
    pub fn change(&self) -> Option<(f64, f64)> {
        let now = self.latest().result.value?;
        let before = self.previous()?.result.value?;
        let pct = if before != 0.0 {
            (now - before) / before.abs() * 100.0
        } else {
            0.0
        };
        Some((now - before, pct))
    }

    /// The labs of the previous and latest tests, when they differ. Methods
    /// vary between labs, so part of such a change may not be biological.
    pub fn lab_change(&self) -> Option<(&str, &str)> {
        let before = self.previous()?;
        let now = self.latest();
        (before.lab != now.lab).then_some((before.lab.as_str(), now.lab.as_str()))
    }

    pub fn values(&self) -> Vec<(NaiveDate, f64)> {
        self.points
            .iter()
            .filter_map(|m| m.result.value.map(|v| (m.date, v)))
            .collect()
    }
}

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let lower = w.to_lowercase();
            let mut c = lower.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Groups measurements into series ordered by panel and catalog order.
pub fn build_series(measurements: &[Measurement], targets: &[(String, Range)]) -> Vec<Series> {
    let targets: HashMap<&str, Range> = targets.iter().map(|(k, r)| (k.as_str(), *r)).collect();
    let mut by_key: BTreeMap<String, Vec<Measurement>> = BTreeMap::new();
    for m in measurements {
        let points = by_key.entry(m.result.biomarker.clone()).or_default();
        // Keep a single value per date (the latest imported wins).
        if let Some(last) = points.last_mut()
            && last.date == m.date
        {
            *last = m.clone();
            continue;
        }
        points.push(m.clone());
    }
    let mut series: Vec<Series> = by_key
        .into_iter()
        .map(|(key, mut points)| {
            points.sort_by_key(|m| m.date);
            Series {
                def: catalog::get(&key),
                original_name: points[0].result.original_name.clone(),
                target: targets.get(key.as_str()).copied(),
                key,
                points,
            }
        })
        .collect();
    let order = |s: &Series| {
        let idx = catalog::CATALOG
            .iter()
            .position(|b| Some(b.key) == s.def.map(|d| d.key))
            .unwrap_or(usize::MAX);
        (s.panel(), idx, s.original_name.clone())
    };
    series.sort_by_key(order);
    series
}

/// Series whose latest value differs from the previous one by at least
/// [`BIG_CHANGE_PCT`].
pub fn big_changes(series: &[Series]) -> Vec<(&Series, f64)> {
    series
        .iter()
        .filter_map(|s| {
            let (_, pct) = s.change()?;
            (pct.abs() >= BIG_CHANGE_PCT).then_some((s, pct))
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Derived {
    pub name: (&'static str, &'static str),
    pub value: f64,
    pub unit: &'static str,
    pub date: NaiveDate,
    pub note: Key,
    pub status: Status,
}

pub fn bmi(weight_kg: f64, height_cm: f64) -> f64 {
    let m = height_cm / 100.0;
    weight_kg / (m * m)
}

pub fn bmi_class(bmi: f64) -> Key {
    match bmi {
        b if b < 18.5 => Key::BmiUnder,
        b if b < 25.0 => Key::BmiNormal,
        b if b < 30.0 => Key::BmiOver,
        _ => Key::BmiObese,
    }
}

/// eGFR by the race-free CKD-EPI 2021 creatinine equation (mL/min/1.73m²).
pub fn egfr_ckd_epi_2021(creatinine_mg_dl: f64, age: u32, sex: Sex) -> f64 {
    let (kappa, alpha, factor) = match sex {
        Sex::Female => (0.7, -0.241, 1.012),
        Sex::Male => (0.9, -0.302, 1.0),
    };
    let ratio = creatinine_mg_dl / kappa;
    142.0
        * ratio.min(1.0).powf(alpha)
        * ratio.max(1.0).powf(-1.200)
        * 0.9938_f64.powi(age as i32)
        * factor
}

/// Derived metrics computed from the most recent date that has their inputs.
pub fn derived(series: &[Series], profile: &Profile, weights: &[WeightEntry]) -> Vec<Derived> {
    let mut by_date: BTreeMap<NaiveDate, HashMap<&str, f64>> = BTreeMap::new();
    for s in series {
        for (date, v) in s.values() {
            by_date.entry(date).or_default().insert(s.key.as_str(), v);
        }
    }
    let latest_with = |keys: &[&str]| {
        by_date
            .iter()
            .rev()
            .find(|(_, vals)| keys.iter().all(|k| vals.contains_key(k)))
            .map(|(d, vals)| (*d, vals))
    };

    let mut out = Vec::new();
    if let Some(w) = weights.last() {
        let value = bmi(w.kg, profile.height_cm);
        out.push(Derived {
            name: ("IMC", "BMI"),
            value,
            unit: "kg/m²",
            date: w.date,
            note: bmi_class(value),
            status: Status::of(
                Some(value),
                &Range {
                    min: Some(18.5),
                    max: Some(24.9),
                },
            ),
        });
    }
    if let Some((date, v)) = latest_with(&["creatinine"]) {
        let value = egfr_ckd_epi_2021(v["creatinine"], profile.age_on(date), profile.sex);
        out.push(Derived {
            name: ("TFG estimada", "eGFR"),
            value,
            unit: "mL/min/1,73m²",
            date,
            note: Key::EgfrNote,
            status: Status::of(
                Some(value),
                &Range {
                    min: Some(90.0),
                    max: None,
                },
            ),
        });
    }
    if let Some((date, v)) = latest_with(&["total_cholesterol", "hdl"]) {
        let value = v["total_cholesterol"] - v["hdl"];
        out.push(Derived {
            name: ("Colesterol não-HDL", "Non-HDL cholesterol"),
            value,
            unit: "mg/dL",
            date,
            note: Key::NonHdlNote,
            status: Status::of(
                Some(value),
                &Range {
                    min: None,
                    max: Some(130.0),
                },
            ),
        });
        if !v.contains_key("ldl")
            && let Some(&tg) = v.get("triglycerides")
            && tg < 400.0
        {
            let value = v["total_cholesterol"] - v["hdl"] - tg / 5.0;
            out.push(Derived {
                name: ("LDL calculado", "Calculated LDL"),
                value,
                unit: "mg/dL",
                date,
                note: Key::FriedewaldNote,
                status: Status::of(
                    Some(value),
                    &Range {
                        min: None,
                        max: Some(130.0),
                    },
                ),
            });
        }
    }
    if let Some((date, v)) = latest_with(&["triglycerides", "hdl"])
        && v["hdl"] > 0.0
    {
        let value = v["triglycerides"] / v["hdl"];
        out.push(Derived {
            name: ("Triglicerídeos / HDL", "Triglycerides / HDL"),
            value,
            unit: "",
            date,
            note: Key::TgHdlNote,
            status: Status::of(
                Some(value),
                &Range {
                    min: None,
                    max: Some(3.0),
                },
            ),
        });
    }
    if let Some((date, v)) = latest_with(&["glucose", "insulin"]) {
        let value = v["glucose"] * v["insulin"] / 405.0;
        out.push(Derived {
            name: ("HOMA-IR", "HOMA-IR"),
            value,
            unit: "",
            date,
            note: Key::HomaNote,
            status: Status::of(
                Some(value),
                &Range {
                    min: None,
                    max: Some(2.7),
                },
            ),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LabResult;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn m(
        key: &str,
        date: NaiveDate,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
    ) -> Measurement {
        Measurement {
            report_id: 1,
            date,
            lab: "Lab A".into(),
            result: LabResult {
                biomarker: key.into(),
                original_name: key.to_uppercase(),
                exam: String::new(),
                value: Some(value),
                text_value: None,
                unit: "mg/dL".into(),
                reference: Range { min, max },
                reference_text: String::new(),
            },
        }
    }

    fn profile() -> Profile {
        Profile {
            name: "X".into(),
            birth_date: d(1992, 3, 25),
            sex: Sex::Male,
            height_cm: 180.0,
        }
    }

    #[test]
    fn status_against_range() {
        let r = Range {
            min: Some(60.0),
            max: Some(99.0),
        };
        assert_eq!(Status::of(Some(59.0), &r), Status::Low);
        assert_eq!(Status::of(Some(99.0), &r), Status::Normal);
        assert_eq!(Status::of(Some(100.0), &r), Status::High);
        assert_eq!(Status::of(None, &r), Status::Unknown);
        assert_eq!(Status::of(Some(1.0), &Range::default()), Status::Unknown);
    }

    #[test]
    fn builds_series_with_changes() {
        let ms = vec![
            m("glucose", d(2024, 7, 19), 77.0, Some(60.0), Some(99.0)),
            m("hemoglobin", d(2024, 7, 19), 16.2, Some(13.9), Some(16.3)),
            m("glucose", d(2025, 9, 9), 104.0, Some(60.0), Some(99.0)),
        ];
        let series = build_series(
            &ms,
            &[(
                "glucose".into(),
                Range {
                    min: None,
                    max: Some(90.0),
                },
            )],
        );
        assert_eq!(series.len(), 2);
        // Blood count panel sorts before glucose.
        assert_eq!(series[0].key, "hemoglobin");
        let glu = &series[1];
        assert_eq!(glu.status(), Status::High);
        assert_eq!(glu.target_status(), Some(Status::High));
        let (abs, pct) = glu.change().unwrap();
        assert_eq!(abs, 27.0);
        assert!((pct - 35.06).abs() < 0.01);
        assert_eq!(big_changes(&series).len(), 1);
        assert!(series[0].change().is_none());
        assert!(glu.lab_change().is_none());
        assert!(series[0].lab_change().is_none());
    }

    #[test]
    fn tells_goal_ranges_from_normal_ranges() {
        assert!(is_goal_range(
            "Desejável: > 60 mg/dL\nBaixo....: < 40 mg/dL"
        ));
        assert!(is_goal_range(
            "Ótimo.....:      < 100 mg/dL\nDesejável.:  100 a 129 mg/dL"
        ));
        assert!(is_goal_range(
            "Sem risco    Risco moderado   Risco elevado\nHomens   > 55 mg/dL    35-55 mg/dL       < 35 mg/dL"
        ));
        assert!(is_goal_range("Risco baixo.....: Abaixo de 130 mg/dL"));
        assert!(!is_goal_range(
            "normal........................: DE  60 a  99 mg/dL"
        ));
        assert!(!is_goal_range("Intervalo normal: < 150 mg/dL"));
        assert!(!is_goal_range(
            "DEFICIENTE: INFERIOR A 20,0 ng/mL\nADEQUADO PARA A POPULAÇÃO GERAL: ENTRE 20,0 A 60,0 ng/mL\nIDEAL (GRUPOS DE RISCO): DE 30,0 A 60,0 ng/mL"
        ));
        assert!(!is_goal_range("30 a 400 ng/mL"));
    }

    #[test]
    fn detects_lab_change() {
        let mut later = m("glucose", d(2025, 9, 9), 104.0, None, None);
        later.lab = "Lab B".into();
        let ms = vec![m("glucose", d(2024, 7, 19), 77.0, None, None), later];
        let series = build_series(&ms, &[]);
        assert_eq!(series[0].lab_change(), Some(("Lab A", "Lab B")));
    }

    #[test]
    fn egfr_matches_reference_values() {
        // 30-year-old man, creatinine 1.0 mg/dL → ~104 mL/min/1.73m².
        let e = egfr_ckd_epi_2021(1.0, 30, Sex::Male);
        assert!((e - 103.8).abs() < 0.5, "{e}");
        // 60-year-old woman, creatinine 1.2 mg/dL → ~51.
        let e = egfr_ckd_epi_2021(1.2, 60, Sex::Female);
        assert!((e - 51.0).abs() < 1.0, "{e}");
    }

    #[test]
    fn bmi_and_weights() {
        assert!((bmi(81.0, 180.0) - 25.0).abs() < 1e-9);
        assert_eq!(bmi_class(24.9), Key::BmiNormal);
        assert_eq!(bmi_class(30.0), Key::BmiObese);
    }

    #[test]
    fn derived_metrics() {
        let date = d(2024, 7, 19);
        let ms = vec![
            m("total_cholesterol", date, 144.0, None, None),
            m("hdl", date, 56.0, None, None),
            m("triglycerides", date, 60.0, None, None),
            m("creatinine", date, 1.0, None, None),
        ];
        let series = build_series(&ms, &[]);
        let w = vec![WeightEntry { date, kg: 81.0 }];
        let out = derived(&series, &profile(), &w);
        let get = |en: &str| out.iter().find(|x| x.name.1 == en).unwrap().value;
        assert!((get("BMI") - 25.0).abs() < 1e-9);
        assert_eq!(get("Non-HDL cholesterol"), 88.0);
        assert_eq!(get("Calculated LDL"), 76.0);
        assert!((get("Triglycerides / HDL") - 60.0 / 56.0).abs() < 1e-9);
        assert!(get("eGFR") > 100.0);
        assert!(out.iter().all(|x| x.name.1 != "HOMA-IR"));
    }
}
