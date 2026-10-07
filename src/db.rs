//! SQLite persistence. Everything lives in the user's data directory
//! (`~/.local/share/b-health` by default, or `$B_HEALTH_DATA_DIR`).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{Local, NaiveDate};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::i18n::Lang;
use crate::model::{LabResult, Measurement, Profile, Report, Sex, WeightEntry};
use crate::parser::ParsedReport;
use crate::units::Range;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    name TEXT NOT NULL,
    birth_date TEXT NOT NULL,
    sex TEXT NOT NULL,
    height_cm REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS weights (
    id INTEGER PRIMARY KEY,
    date TEXT NOT NULL UNIQUE,
    kg REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS reports (
    id INTEGER PRIMARY KEY,
    lab TEXT NOT NULL,
    order_no TEXT,
    collected_on TEXT NOT NULL,
    imported_at TEXT NOT NULL,
    file_hash TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    stored_path TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS results (
    id INTEGER PRIMARY KEY,
    report_id INTEGER NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
    biomarker TEXT NOT NULL,
    original_name TEXT NOT NULL,
    exam TEXT NOT NULL,
    value REAL,
    text_value TEXT,
    unit TEXT NOT NULL,
    ref_min REAL,
    ref_max REAL,
    ref_text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS results_biomarker ON results(biomarker);
CREATE TABLE IF NOT EXISTS targets (
    biomarker TEXT PRIMARY KEY,
    min REAL,
    max REAL
);
";

const DATE_FMT: &str = "%Y-%m-%d";

pub struct Db {
    conn: Connection,
    dir: PathBuf,
}

pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("B_HEALTH_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "b-health")
        .map(|d| d.data_dir().to_path_buf())
        .ok_or_else(|| anyhow!("could not determine the data directory"))
}

pub fn file_hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn date(s: String) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(&s, DATE_FMT).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

impl Db {
    pub fn open_default() -> Result<Db> {
        Db::open(&data_dir()?)
    }

    pub fn open(dir: &Path) -> Result<Db> {
        std::fs::create_dir_all(dir.join("docs"))
            .with_context(|| format!("could not create {}", dir.display()))?;
        let conn = Connection::open(dir.join("b-health.db"))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Db {
            conn,
            dir: dir.to_path_buf(),
        })
    }

    #[cfg(test)]
    pub fn in_memory() -> Db {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        Db {
            conn,
            dir: std::env::temp_dir(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    // --- profile & settings ---------------------------------------------------

    pub fn profile(&self) -> Result<Option<Profile>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name, birth_date, sex, height_cm FROM profile WHERE id = 1",
                [],
                |r| {
                    Ok(Profile {
                        name: r.get(0)?,
                        birth_date: date(r.get(1)?)?,
                        sex: Sex::from_code(&r.get::<_, String>(2)?).unwrap_or(Sex::Male),
                        height_cm: r.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn save_profile(&self, p: &Profile) -> Result<()> {
        self.conn.execute(
            "INSERT INTO profile (id, name, birth_date, sex, height_cm) VALUES (1, ?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET name = ?1, birth_date = ?2, sex = ?3, height_cm = ?4",
            params![
                p.name,
                p.birth_date.format(DATE_FMT).to_string(),
                p.sex.code(),
                p.height_cm
            ],
        )?;
        Ok(())
    }

    pub fn lang(&self) -> Result<Lang> {
        let code: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = 'lang'", [], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(code.map(|c| Lang::from_code(&c)).unwrap_or_default())
    }

    pub fn set_lang(&self, lang: Lang) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES ('lang', ?1)
             ON CONFLICT(key) DO UPDATE SET value = ?1",
            [lang.code()],
        )?;
        Ok(())
    }

    // --- weights ----------------------------------------------------------------

    /// Records a weight; a second entry on the same day replaces the first.
    pub fn add_weight(&self, date: NaiveDate, kg: f64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO weights (date, kg) VALUES (?1, ?2)
             ON CONFLICT(date) DO UPDATE SET kg = ?2",
            params![date.format(DATE_FMT).to_string(), kg],
        )?;
        Ok(())
    }

    /// All weights, oldest first.
    pub fn weights(&self) -> Result<Vec<WeightEntry>> {
        let mut stmt = self
            .conn
            .prepare("SELECT date, kg FROM weights ORDER BY date")?;
        let rows = stmt.query_map([], |r| {
            Ok(WeightEntry {
                date: date(r.get(0)?)?,
                kg: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // --- reports & results ---------------------------------------------------

    pub fn has_hash(&self, hash: &str) -> Result<bool> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM reports WHERE file_hash = ?1", [hash], |_| {
                Ok(())
            })
            .optional()?
            .is_some())
    }

    /// Stores a reviewed report, its results and a copy of the original PDF.
    pub fn save_report(
        &mut self,
        parsed: &ParsedReport,
        collected_on: NaiveDate,
        results: &[LabResult],
        pdf: &[u8],
        file_name: &str,
    ) -> Result<i64> {
        let hash = file_hash(pdf);
        let stored = self.dir.join("docs").join(format!(
            "{}-{}.pdf",
            collected_on.format(DATE_FMT),
            &hash[..12]
        ));
        std::fs::write(&stored, pdf)
            .with_context(|| format!("could not copy the PDF to {}", stored.display()))?;

        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO reports (lab, order_no, collected_on, imported_at, file_hash, file_name, stored_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                parsed.lab,
                parsed.order_no,
                collected_on.format(DATE_FMT).to_string(),
                Local::now().to_rfc3339(),
                hash,
                file_name,
                stored.to_string_lossy(),
            ],
        )?;
        let report_id = tx.last_insert_rowid();
        {
            let mut stmt = tx.prepare(
                "INSERT INTO results (report_id, biomarker, original_name, exam, value, text_value,
                                      unit, ref_min, ref_max, ref_text)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            )?;
            for r in results {
                stmt.execute(params![
                    report_id,
                    r.biomarker,
                    r.original_name,
                    r.exam,
                    r.value,
                    r.text_value,
                    r.unit,
                    r.reference.min,
                    r.reference.max,
                    r.reference_text,
                ])?;
            }
        }
        tx.commit()?;
        Ok(report_id)
    }

    /// All reports, newest first, with their result count.
    pub fn reports(&self) -> Result<Vec<(Report, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.lab, r.order_no, r.collected_on, r.file_name,
                    (SELECT COUNT(*) FROM results WHERE report_id = r.id)
             FROM reports r ORDER BY r.collected_on DESC, r.id DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                Report {
                    id: r.get(0)?,
                    lab: r.get(1)?,
                    order_no: r.get(2)?,
                    collected_on: date(r.get(3)?)?,
                    file_name: r.get(4)?,
                },
                r.get::<_, i64>(5)? as usize,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn delete_report(&self, id: i64) -> Result<()> {
        let path: Option<String> = self
            .conn
            .query_row("SELECT stored_path FROM reports WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()?;
        self.conn
            .execute("DELETE FROM reports WHERE id = ?1", [id])?;
        if let Some(path) = path {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }

    /// Every stored result, oldest report first.
    pub fn measurements(&self) -> Result<Vec<Measurement>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.report_id, p.collected_on, r.biomarker, r.original_name, r.exam, r.value,
                    r.text_value, r.unit, r.ref_min, r.ref_max, r.ref_text, p.lab
             FROM results r JOIN reports p ON p.id = r.report_id
             ORDER BY p.collected_on, r.report_id, r.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Measurement {
                report_id: r.get(0)?,
                date: date(r.get(1)?)?,
                lab: r.get(11)?,
                result: LabResult {
                    biomarker: r.get(2)?,
                    original_name: r.get(3)?,
                    exam: r.get(4)?,
                    value: r.get(5)?,
                    text_value: r.get(6)?,
                    unit: r.get(7)?,
                    reference: Range {
                        min: r.get(8)?,
                        max: r.get(9)?,
                    },
                    reference_text: r.get(10)?,
                },
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    // --- targets ----------------------------------------------------------------

    pub fn targets(&self) -> Result<Vec<(String, Range)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT biomarker, min, max FROM targets")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                Range {
                    min: r.get(1)?,
                    max: r.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn set_target(&self, biomarker: &str, range: Option<Range>) -> Result<()> {
        match range {
            Some(r) => self.conn.execute(
                "INSERT INTO targets (biomarker, min, max) VALUES (?1, ?2, ?3)
                 ON CONFLICT(biomarker) DO UPDATE SET min = ?2, max = ?3",
                params![biomarker, r.min, r.max],
            )?,
            None => self
                .conn
                .execute("DELETE FROM targets WHERE biomarker = ?1", [biomarker])?,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn profile_roundtrip() {
        let db = Db::in_memory();
        assert!(db.profile().unwrap().is_none());
        let p = Profile {
            name: "Ana".into(),
            birth_date: d(1990, 1, 2),
            sex: Sex::Female,
            height_cm: 165.0,
        };
        db.save_profile(&p).unwrap();
        db.save_profile(&Profile {
            height_cm: 166.0,
            ..p.clone()
        })
        .unwrap();
        let got = db.profile().unwrap().unwrap();
        assert_eq!(got.name, "Ana");
        assert_eq!(got.sex, Sex::Female);
        assert_eq!(got.height_cm, 166.0);
    }

    #[test]
    fn weights_replace_same_day() {
        let db = Db::in_memory();
        db.add_weight(d(2025, 1, 1), 80.0).unwrap();
        db.add_weight(d(2025, 1, 1), 79.5).unwrap();
        db.add_weight(d(2024, 6, 1), 82.0).unwrap();
        let w = db.weights().unwrap();
        assert_eq!(w.len(), 2);
        assert_eq!(w[0].kg, 82.0);
        assert_eq!(w[1].kg, 79.5);
    }

    #[test]
    fn report_lifecycle() {
        let dir = std::env::temp_dir().join(format!("b-health-test-{}", std::process::id()));
        let mut db = Db::open(&dir).unwrap();
        let parsed = ParsedReport {
            lab: "Lab".into(),
            ..Default::default()
        };
        let result = LabResult {
            biomarker: "glucose".into(),
            original_name: "GLICOSE".into(),
            exam: "GLICOSE".into(),
            value: Some(90.0),
            text_value: None,
            unit: "mg/dL".into(),
            reference: Range {
                min: Some(60.0),
                max: Some(99.0),
            },
            reference_text: "60 a 99".into(),
        };
        let pdf = b"%PDF-fake";
        let id = db
            .save_report(
                &parsed,
                d(2025, 9, 9),
                std::slice::from_ref(&result),
                pdf,
                "a.pdf",
            )
            .unwrap();
        assert!(db.has_hash(&file_hash(pdf)).unwrap());
        let stored = &db.measurements().unwrap()[0];
        assert_eq!(stored.result, result);
        assert_eq!(stored.lab, "Lab");
        assert_eq!(db.reports().unwrap()[0].1, 1);
        db.delete_report(id).unwrap();
        assert!(db.measurements().unwrap().is_empty());
        assert!(!db.has_hash(&file_hash(pdf)).unwrap());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
