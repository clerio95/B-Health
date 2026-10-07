//! CSV and JSON export of every stored result.

use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use serde_json::json;

use crate::db::Db;

const CSV_HEADER: &str =
    "date,biomarker,original_name,exam,value,text_value,unit,ref_min,ref_max,ref_text";

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn opt(v: Option<f64>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

pub fn to_csv(db: &Db) -> Result<String> {
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for m in db.measurements()? {
        let r = &m.result;
        let fields = [
            m.date.to_string(),
            csv_field(&r.biomarker),
            csv_field(&r.original_name),
            csv_field(&r.exam),
            opt(r.value),
            csv_field(r.text_value.as_deref().unwrap_or("")),
            csv_field(&r.unit),
            opt(r.reference.min),
            opt(r.reference.max),
            csv_field(&r.reference_text),
        ];
        out.push_str(&fields.join(","));
        out.push('\n');
    }
    Ok(out)
}

pub fn to_json(db: &Db) -> Result<String> {
    let profile = db.profile()?.map(|p| {
        json!({
            "name": p.name,
            "birth_date": p.birth_date.to_string(),
            "sex": p.sex.code(),
            "height_cm": p.height_cm,
        })
    });
    let weights: Vec<_> = db
        .weights()?
        .into_iter()
        .map(|w| json!({ "date": w.date.to_string(), "kg": w.kg }))
        .collect();
    let reports: Vec<_> = db
        .reports()?
        .into_iter()
        .map(|(r, _)| {
            json!({
                "id": r.id,
                "lab": r.lab,
                "order_no": r.order_no,
                "collected_on": r.collected_on.to_string(),
                "file_name": r.file_name,
            })
        })
        .collect();
    let results: Vec<_> = db
        .measurements()?
        .into_iter()
        .map(|m| {
            let r = m.result;
            json!({
                "report_id": m.report_id,
                "date": m.date.to_string(),
                "biomarker": r.biomarker,
                "original_name": r.original_name,
                "exam": r.exam,
                "value": r.value,
                "text_value": r.text_value,
                "unit": r.unit,
                "reference": { "min": r.reference.min, "max": r.reference.max, "text": r.reference_text },
            })
        })
        .collect();
    let targets: Vec<_> = db
        .targets()?
        .into_iter()
        .map(|(k, r)| json!({ "biomarker": k, "min": r.min, "max": r.max }))
        .collect();
    Ok(serde_json::to_string_pretty(&json!({
        "profile": profile,
        "weights": weights,
        "reports": reports,
        "results": results,
        "targets": targets,
    }))?)
}

/// Writes CSV and JSON exports into `<data dir>/exports`; returns the folder.
pub fn export_files(db: &Db) -> Result<PathBuf> {
    let dir = db.dir().join("exports");
    std::fs::create_dir_all(&dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    std::fs::write(dir.join(format!("b-health-{stamp}.csv")), to_csv(db)?)?;
    std::fs::write(dir.join(format!("b-health-{stamp}.json")), to_json(db)?)?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_csv_fields() {
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("plain"), "plain");
    }

    #[test]
    fn exports_empty_db() {
        let db = Db::in_memory();
        assert_eq!(to_csv(&db).unwrap().lines().count(), 1);
        assert!(to_json(&db).unwrap().contains("\"results\": []"));
    }
}
