mod app;
mod catalog;
mod db;
mod export;
mod i18n;
mod insights;
mod model;
mod parser;
mod pdf_text;
mod ui;
mod units;

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::app::App;
use crate::db::Db;

const USAGE: &str = "\
B-Health — acompanhe seus exames de sangue / track your blood tests

Uso / Usage:
  b-health                  abre o aplicativo / open the app
  b-health import <pdf>     importa um laudo / import a report
  b-health parse <pdf>      mostra o que foi lido do PDF, sem salvar / show parsed results without saving
  b-health text <pdf>       mostra o texto extraído do PDF / show the extracted PDF text
  b-health export csv|json  exporta os dados para a saída padrão / export data to stdout

Dados / Data: ~/.local/share/b-health (ou/or $B_HEALTH_DATA_DIR)";

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = run(&args.iter().map(String::as_str).collect::<Vec<_>>());
    // Output piped into `head` and the like: the reader leaving is not an error.
    match result {
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe) =>
        {
            Ok(())
        }
        other => other,
    }
}

fn run(args: &[&str]) -> Result<()> {
    let mut out = io::stdout().lock();
    match args {
        [] => run_tui(None),
        ["import", path] => run_tui(Some(PathBuf::from(path))),
        ["parse", path] => print_parsed(&mut out, path),
        ["text", path] => {
            let bytes = std::fs::read(path).with_context(|| format!("could not read {path}"))?;
            for (i, page) in pdf_text::extract_pages(&bytes)?.iter().enumerate() {
                writeln!(out, "===== {} =====\n{page}", i + 1)?;
            }
            Ok(())
        }
        ["export", "csv"] => Ok(write!(out, "{}", export::to_csv(&Db::open_default()?)?)?),
        ["export", "json"] => Ok(writeln!(out, "{}", export::to_json(&Db::open_default()?)?)?),
        ["-h" | "--help" | "help"] => Ok(writeln!(out, "{USAGE}")?),
        _ => bail!("argumentos inválidos / invalid arguments\n\n{USAGE}"),
    }
}

fn print_parsed(out: &mut impl Write, path: &str) -> Result<()> {
    let bytes = std::fs::read(path).with_context(|| format!("could not read {path}"))?;
    let profile = Db::open_default()
        .ok()
        .and_then(|db| db.profile().ok().flatten());
    let report = parser::parse_pdf(&bytes, &profile.as_ref().into())?;
    writeln!(out, "Lab:        {}", report.lab)?;
    writeln!(
        out,
        "Order:      {}",
        report.order_no.as_deref().unwrap_or("-")
    )?;
    writeln!(
        out,
        "Collected:  {}",
        report
            .collected_on
            .map(|d| d.to_string())
            .unwrap_or("-".into())
    )?;
    writeln!(
        out,
        "Patient:    {} / {} / {}",
        report.patient_name.as_deref().unwrap_or("-"),
        report
            .birth_date
            .map(|d| d.to_string())
            .unwrap_or("-".into()),
        report.sex.map(|s| s.code()).unwrap_or("-")
    )?;
    writeln!(out)?;
    writeln!(
        out,
        "{:<22} {:<34} {:>12} {:<12} {:>9} {:>9}",
        "biomarker", "original name", "value", "unit", "ref min", "ref max"
    )?;
    let opt = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
    for r in &report.results {
        let value = match (&r.value, &r.text_value) {
            (Some(v), _) => v.to_string(),
            (None, Some(t)) => t.clone(),
            _ => String::new(),
        };
        writeln!(
            out,
            "{:<22} {:<34} {:>12} {:<12} {:>9} {:>9}",
            r.biomarker,
            r.original_name,
            value,
            r.unit,
            opt(r.reference.min),
            opt(r.reference.max)
        )?;
    }
    writeln!(out, "\n{} results", report.results.len())?;
    Ok(())
}

fn run_tui(import: Option<PathBuf>) -> Result<()> {
    let db = Db::open_default()?;
    let mut app = App::new(db, import)?;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        while !app.quit {
            terminal.draw(|f| ui::draw(f, &app))?;
            if let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.handle_key(key);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
