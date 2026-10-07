//! Application state and input handling for the TUI.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Local, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::catalog;
use crate::db::{self, Db};
use crate::i18n::{Key, Lang};
use crate::insights::{self, Series};
use crate::model::{LabResult, Profile, Report, Sex, WeightEntry};
use crate::parser::{self, ParsedReport};
use crate::units::{Range, parse_br_number};

// --- small text input -----------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub value: String,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> TextInput {
        TextInput {
            value: value.into(),
        }
    }

    /// Applies an editing key. Returns true if the key was consumed.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.push(c);
                true
            }
            KeyCode::Backspace => {
                self.value.pop();
                true
            }
            _ => false,
        }
    }

    /// Like [`handle`], inserting `/` automatically for `DD/MM/YYYY`.
    pub fn handle_date(&mut self, key: KeyEvent) -> bool {
        if let KeyCode::Char(c) = key.code
            && c.is_ascii_digit()
            && matches!(self.value.len(), 2 | 5)
        {
            self.value.push('/');
        }
        self.handle(key)
    }
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%d/%m/%Y").ok()
}

pub fn parse_decimal(s: &str) -> Option<f64> {
    parse_br_number(s.trim()).filter(|v| v.is_finite())
}

/// Parses a target like `30-60`, `-60` (max only) or `30-` (min only).
pub fn parse_target(s: &str) -> Option<Option<Range>> {
    let s = s.trim();
    if s.is_empty() {
        return Some(None);
    }
    let (min, max) = s.split_once('-')?;
    let part = |p: &str| -> Option<Option<f64>> {
        let p = p.trim();
        if p.is_empty() {
            Some(None)
        } else {
            parse_decimal(p).map(Some)
        }
    };
    let range = Range {
        min: part(min)?,
        max: part(max)?,
    };
    if range.is_empty() || matches!((range.min, range.max), (Some(a), Some(b)) if a > b) {
        return None;
    }
    Some(Some(range))
}

// --- onboarding / profile form -------------------------------------------------

pub const FIELD_NAME: usize = 0;
pub const FIELD_BIRTH: usize = 1;
pub const FIELD_SEX: usize = 2;
pub const FIELD_WEIGHT: usize = 3;
pub const FIELD_HEIGHT: usize = 4;
pub const FIELD_COUNT: usize = 5;

#[derive(Debug, Clone)]
pub struct ProfileForm {
    pub name: TextInput,
    pub birth: TextInput,
    pub sex: Sex,
    pub weight: TextInput,
    pub height: TextInput,
    pub focus: usize,
    pub errors: [Option<Key>; FIELD_COUNT],
    /// Editing an existing profile (vs. first run).
    pub editing: bool,
    initial_weight: Option<f64>,
}

impl ProfileForm {
    pub fn new() -> ProfileForm {
        ProfileForm {
            name: TextInput::default(),
            birth: TextInput::default(),
            sex: Sex::Male,
            weight: TextInput::default(),
            height: TextInput::default(),
            focus: 0,
            errors: Default::default(),
            editing: false,
            initial_weight: None,
        }
    }

    pub fn edit(profile: &Profile, weight: Option<f64>, lang: Lang) -> ProfileForm {
        let num = |v: f64| crate::units::format_number(v, lang.decimal_comma());
        ProfileForm {
            name: TextInput::new(&profile.name),
            birth: TextInput::new(profile.birth_date.format("%d/%m/%Y").to_string()),
            sex: profile.sex,
            weight: TextInput::new(weight.map(num).unwrap_or_default()),
            height: TextInput::new(num(profile.height_cm)),
            focus: 0,
            errors: Default::default(),
            editing: true,
            initial_weight: weight,
        }
    }

    /// Validates every field; returns the profile and weight when valid.
    pub fn validate(&mut self, today: NaiveDate) -> Option<(Profile, f64)> {
        self.errors = Default::default();
        let name = self.name.value.trim().to_string();
        if name.is_empty() {
            self.errors[FIELD_NAME] = Some(Key::ErrName);
        }
        let birth = parse_date(&self.birth.value);
        match birth {
            None => self.errors[FIELD_BIRTH] = Some(Key::ErrDate),
            Some(d) if d > today => self.errors[FIELD_BIRTH] = Some(Key::ErrDateFuture),
            _ => {}
        }
        let weight = parse_decimal(&self.weight.value).filter(|w| (2.0..=400.0).contains(w));
        if weight.is_none() {
            self.errors[FIELD_WEIGHT] = Some(Key::ErrWeight);
        }
        let height = parse_decimal(&self.height.value).filter(|h| (40.0..=250.0).contains(h));
        if height.is_none() {
            self.errors[FIELD_HEIGHT] = Some(Key::ErrHeight);
        }
        if let Some(first) = self.errors.iter().position(Option::is_some) {
            self.focus = first;
            return None;
        }
        Some((
            Profile {
                name,
                birth_date: birth?,
                sex: self.sex,
                height_cm: height?,
            },
            weight?,
        ))
    }
}

// --- file browser --------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FileBrowser {
    pub dir: PathBuf,
    /// (name, is_dir), directories first.
    pub entries: Vec<(String, bool)>,
    pub selected: usize,
    pub show_hidden: bool,
}

impl FileBrowser {
    pub fn new() -> FileBrowser {
        let home = directories::UserDirs::new();
        let dir = home
            .as_ref()
            .and_then(|u| u.download_dir().map(Path::to_path_buf))
            .filter(|d| d.is_dir())
            .or_else(|| home.as_ref().map(|u| u.home_dir().to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let mut b = FileBrowser {
            dir,
            entries: Vec::new(),
            selected: 0,
            show_hidden: false,
        };
        b.refresh();
        b
    }

    pub fn refresh(&mut self) {
        let mut entries: Vec<(String, bool)> = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        let is_dir = e.path().is_dir();
                        let is_pdf = name.to_lowercase().ends_with(".pdf");
                        let hidden = name.starts_with('.');
                        ((is_dir || is_pdf) && (self.show_hidden || !hidden))
                            .then_some((name, is_dir))
                    })
                    .collect()
            })
            .unwrap_or_default();
        entries.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });
        if self.dir.parent().is_some() {
            entries.insert(0, ("..".to_string(), true));
        }
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    fn enter_dir(&mut self, dir: PathBuf) {
        let previous = self
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        let going_up = dir == self.dir.parent().map(Path::to_path_buf).unwrap_or_default();
        self.dir = dir;
        self.selected = 0;
        self.refresh();
        if going_up && let Some(prev) = previous {
            self.selected = self.entries.iter().position(|e| e.0 == prev).unwrap_or(0);
        }
    }

    pub fn up(&mut self) {
        if let Some(parent) = self.dir.parent().map(Path::to_path_buf) {
            self.enter_dir(parent);
        }
    }

    /// Opens the selected entry; returns the path when a PDF was chosen.
    pub fn open(&mut self) -> Option<PathBuf> {
        let (name, is_dir) = self.entries.get(self.selected)?.clone();
        if name == ".." {
            self.up();
            None
        } else if is_dir {
            self.enter_dir(self.dir.join(name));
            None
        } else {
            Some(self.dir.join(name))
        }
    }
}

// --- import review -----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Review {
    pub file_name: String,
    pub bytes: Vec<u8>,
    pub parsed: ParsedReport,
    pub rows: Vec<(LabResult, bool)>,
    pub date: NaiveDate,
    pub selected: usize,
    pub warnings: Vec<String>,
}

// --- app -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Biomarkers,
    Insights,
    Reports,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Biomarkers, Tab::Insights, Tab::Reports];

    pub fn title(self, lang: Lang) -> &'static str {
        lang.t(match self {
            Tab::Biomarkers => Key::TabBiomarkers,
            Tab::Insights => Key::TabInsights,
            Tab::Reports => Key::TabReports,
        })
    }
}

pub enum Screen {
    Onboarding(ProfileForm),
    Dashboard,
    Browser(FileBrowser),
    Review(Review),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputKind {
    Weight,
    Target(String),
    ReviewValue(usize),
    ReviewDate,
}

pub enum Popup {
    Input {
        kind: InputKind,
        input: TextInput,
        error: bool,
    },
    ConfirmDelete(i64),
}

pub struct App {
    pub db: Db,
    pub lang: Lang,
    pub profile: Option<Profile>,
    pub weights: Vec<WeightEntry>,
    pub series: Vec<Series>,
    pub reports: Vec<(Report, usize)>,
    pub screen: Screen,
    pub popup: Option<Popup>,
    pub tab: Tab,
    /// Selected series (index into `series`).
    pub selected: usize,
    pub report_selected: usize,
    pub insights_scroll: u16,
    pub status: Option<(String, bool)>,
    pub quit: bool,
    /// File to import right after onboarding (from the command line).
    pending_import: Option<PathBuf>,
}

impl App {
    pub fn new(db: Db, import: Option<PathBuf>) -> Result<App> {
        let lang = db.lang()?;
        let profile = db.profile()?;
        let screen = if profile.is_some() {
            Screen::Dashboard
        } else {
            Screen::Onboarding(ProfileForm::new())
        };
        let mut app = App {
            db,
            lang,
            profile,
            weights: Vec::new(),
            series: Vec::new(),
            reports: Vec::new(),
            screen,
            popup: None,
            tab: Tab::Biomarkers,
            selected: 0,
            report_selected: 0,
            insights_scroll: 0,
            status: None,
            quit: false,
            pending_import: import,
        };
        app.reload()?;
        if app.profile.is_some()
            && let Some(path) = app.pending_import.take()
        {
            app.start_import(&path);
        }
        Ok(app)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.weights = self.db.weights()?;
        self.series = insights::build_series(&self.db.measurements()?, &self.db.targets()?);
        self.reports = self.db.reports()?;
        self.selected = self.selected.min(self.series.len().saturating_sub(1));
        self.report_selected = self
            .report_selected
            .min(self.reports.len().saturating_sub(1));
        Ok(())
    }

    pub fn t(&self, key: Key) -> &'static str {
        self.lang.t(key)
    }

    fn info(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), false));
    }

    fn error(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), true));
    }

    fn toggle_lang(&mut self) {
        self.lang = self.lang.toggle();
        if let Err(e) = self.db.set_lang(self.lang) {
            self.error(e.to_string());
        }
    }

    pub fn current_weight(&self) -> Option<f64> {
        self.weights.last().map(|w| w.kg)
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let result = if self.popup.is_some() {
            self.popup_key(key)
        } else {
            match self.screen {
                Screen::Onboarding(_) => self.onboarding_key(key),
                Screen::Dashboard => self.dashboard_key(key),
                Screen::Browser(_) => self.browser_key(key),
                Screen::Review(_) => self.review_key(key),
            }
        };
        if let Err(e) = result {
            self.error(format!("{e:#}"));
        }
    }

    // --- onboarding ------------------------------------------------------------

    fn onboarding_key(&mut self, key: KeyEvent) -> Result<()> {
        let Screen::Onboarding(form) = &mut self.screen else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                if form.editing {
                    self.screen = Screen::Dashboard;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::F(2) => self.toggle_lang(),
            KeyCode::Tab | KeyCode::Down => form.focus = (form.focus + 1) % FIELD_COUNT,
            KeyCode::BackTab | KeyCode::Up => {
                form.focus = (form.focus + FIELD_COUNT - 1) % FIELD_COUNT
            }
            KeyCode::Enter => {
                let today = Local::now().date_naive();
                if let Some((profile, weight)) = form.validate(today) {
                    let weight_changed = form.initial_weight != Some(weight);
                    self.db.save_profile(&profile)?;
                    if weight_changed {
                        self.db.add_weight(today, weight)?;
                    }
                    self.profile = Some(profile);
                    self.screen = Screen::Dashboard;
                    self.reload()?;
                    if let Some(path) = self.pending_import.take() {
                        self.start_import(&path);
                    }
                }
            }
            _ if form.focus == FIELD_SEX => {
                if matches!(
                    key.code,
                    KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                ) {
                    form.sex = match form.sex {
                        Sex::Male => Sex::Female,
                        Sex::Female => Sex::Male,
                    };
                } else if let KeyCode::Char(c) = key.code {
                    match c.to_ascii_lowercase() {
                        'm' => form.sex = Sex::Male,
                        'f' => form.sex = Sex::Female,
                        _ => {}
                    }
                }
            }
            _ => {
                let focus = form.focus;
                let consumed = match focus {
                    FIELD_NAME => form.name.handle(key),
                    FIELD_BIRTH => form.birth.handle_date(key),
                    FIELD_WEIGHT => form.weight.handle(key),
                    FIELD_HEIGHT => form.height.handle(key),
                    _ => false,
                };
                if consumed {
                    form.errors[focus] = None;
                }
            }
        }
        Ok(())
    }

    // --- dashboard ---------------------------------------------------------------

    fn dashboard_key(&mut self, key: KeyEvent) -> Result<()> {
        self.status = None;
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.quit = true,
            KeyCode::Tab | KeyCode::Right => {
                let i = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.tab = Tab::ALL[(i + 1) % Tab::ALL.len()];
            }
            KeyCode::BackTab | KeyCode::Left => {
                let i = Tab::ALL.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.tab = Tab::ALL[(i + Tab::ALL.len() - 1) % Tab::ALL.len()];
            }
            KeyCode::Char('1') => self.tab = Tab::Biomarkers,
            KeyCode::Char('2') => self.tab = Tab::Insights,
            KeyCode::Char('3') => self.tab = Tab::Reports,
            KeyCode::Char('L') | KeyCode::Char('l') => self.toggle_lang(),
            KeyCode::Char('i') => self.screen = Screen::Browser(FileBrowser::new()),
            KeyCode::Char('w') => {
                self.popup = Some(Popup::Input {
                    kind: InputKind::Weight,
                    input: TextInput::default(),
                    error: false,
                })
            }
            KeyCode::Char('p') => {
                if let Some(p) = &self.profile {
                    self.screen =
                        Screen::Onboarding(ProfileForm::edit(p, self.current_weight(), self.lang));
                }
            }
            KeyCode::Char('e') => {
                let path = crate::export::export_files(&self.db)?;
                self.info(format!("{} {}", self.t(Key::Exported), path.display()));
            }
            KeyCode::Char('t') if self.tab == Tab::Biomarkers => {
                if let Some(s) = self.series.get(self.selected) {
                    let current = s
                        .target
                        .map(|r| {
                            let f = |v: Option<f64>| {
                                v.map(|v| crate::units::format_number(v, self.lang.decimal_comma()))
                                    .unwrap_or_default()
                            };
                            format!("{}-{}", f(r.min), f(r.max))
                        })
                        .unwrap_or_default();
                    self.popup = Some(Popup::Input {
                        kind: InputKind::Target(s.key.clone()),
                        input: TextInput::new(current),
                        error: false,
                    });
                }
            }
            KeyCode::Char('d') | KeyCode::Delete if self.tab == Tab::Reports => {
                if let Some((r, _)) = self.reports.get(self.report_selected) {
                    self.popup = Some(Popup::ConfirmDelete(r.id));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home => self.move_selection(-10_000),
            KeyCode::End => self.move_selection(10_000),
            _ => {}
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: i32) {
        let step = |cur: usize, len: usize| -> usize {
            (cur as i64 + delta as i64).clamp(0, len.saturating_sub(1) as i64) as usize
        };
        match self.tab {
            Tab::Biomarkers => self.selected = step(self.selected, self.series.len()),
            Tab::Reports => self.report_selected = step(self.report_selected, self.reports.len()),
            Tab::Insights => {
                self.insights_scroll =
                    (self.insights_scroll as i32 + delta).clamp(0, u16::MAX as i32) as u16
            }
        }
    }

    // --- popups ------------------------------------------------------------------

    fn popup_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(popup) = self.popup.as_mut() else {
            return Ok(());
        };
        match popup {
            Popup::ConfirmDelete(id) => {
                let id = *id;
                self.popup = None;
                if matches!(key.code, KeyCode::Char('s' | 'S' | 'y' | 'Y')) {
                    self.db.delete_report(id)?;
                    self.reload()?;
                    self.info(self.t(Key::Deleted));
                }
            }
            Popup::Input { kind, input, error } => match key.code {
                KeyCode::Esc => self.popup = None,
                KeyCode::Enter => {
                    let kind = kind.clone();
                    let text = input.value.clone();
                    if self.submit_input(&kind, &text)? {
                        self.popup = None;
                    } else if let Some(Popup::Input { error, .. }) = self.popup.as_mut() {
                        *error = true;
                    }
                }
                _ => {
                    let consumed = if *kind == InputKind::ReviewDate {
                        input.handle_date(key)
                    } else {
                        input.handle(key)
                    };
                    if consumed {
                        *error = false;
                    }
                }
            },
        }
        Ok(())
    }

    /// Applies a popup's input. Returns false if the input is invalid.
    fn submit_input(&mut self, kind: &InputKind, text: &str) -> Result<bool> {
        match kind {
            InputKind::Weight => {
                let Some(kg) = parse_decimal(text).filter(|w| (2.0..=400.0).contains(w)) else {
                    return Ok(false);
                };
                self.db.add_weight(Local::now().date_naive(), kg)?;
                self.reload()?;
            }
            InputKind::Target(key) => {
                let Some(target) = parse_target(text) else {
                    return Ok(false);
                };
                self.db.set_target(key, target)?;
                self.reload()?;
                if target.is_none() {
                    self.info(self.t(Key::TargetCleared));
                }
            }
            InputKind::ReviewValue(i) => {
                let Some(v) = parse_decimal(text) else {
                    return Ok(false);
                };
                if let Screen::Review(review) = &mut self.screen
                    && let Some((row, _)) = review.rows.get_mut(*i)
                {
                    row.value = Some(v);
                    row.text_value = None;
                }
            }
            InputKind::ReviewDate => {
                let Some(d) = parse_date(text) else {
                    return Ok(false);
                };
                if let Screen::Review(review) = &mut self.screen {
                    review.date = d;
                }
            }
        }
        Ok(true)
    }

    // --- file browser ------------------------------------------------------------

    fn browser_key(&mut self, key: KeyEvent) -> Result<()> {
        let Screen::Browser(b) = &mut self.screen else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Dashboard,
            KeyCode::Up | KeyCode::Char('k') => b.selected = b.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                b.selected = (b.selected + 1).min(b.entries.len().saturating_sub(1))
            }
            KeyCode::PageUp => b.selected = b.selected.saturating_sub(10),
            KeyCode::PageDown => {
                b.selected = (b.selected + 10).min(b.entries.len().saturating_sub(1))
            }
            KeyCode::Backspace | KeyCode::Left | KeyCode::Char('h') => b.up(),
            KeyCode::Char('.') => {
                b.show_hidden = !b.show_hidden;
                b.refresh();
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(path) = b.open() {
                    self.screen = Screen::Dashboard;
                    self.start_import(&path);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Reads and parses a PDF, then opens the review screen.
    pub fn start_import(&mut self, path: &Path) {
        if let Err(e) = self.try_start_import(path) {
            self.screen = Screen::Dashboard;
            self.error(format!("{}: {e:#}", self.t(Key::ParseFailed)));
        }
    }

    fn try_start_import(&mut self, path: &Path) -> Result<()> {
        let bytes = std::fs::read(path)?;
        if self.db.has_hash(&db::file_hash(&bytes))? {
            self.screen = Screen::Dashboard;
            self.error(self.t(Key::Duplicate));
            return Ok(());
        }
        let parsed = parser::parse_pdf(&bytes, &self.profile.as_ref().into())?;
        if parsed.results.is_empty() {
            self.screen = Screen::Dashboard;
            self.error(self.t(Key::NothingFound));
            return Ok(());
        }
        let mut warnings = Vec::new();
        if let Some(p) = &self.profile {
            if let Some(name) = &parsed.patient_name
                && catalog::normalize(name) != catalog::normalize(&p.name)
            {
                warnings.push(format!("{}: {name}", self.t(Key::WarnNameMismatch)));
            }
            if let Some(birth) = parsed.birth_date
                && birth != p.birth_date
            {
                warnings.push(format!(
                    "{}: {}",
                    self.t(Key::WarnBirthMismatch),
                    birth.format(self.lang.date_format())
                ));
            }
            if parsed.sex.is_some_and(|s| s != p.sex) {
                warnings.push(self.t(Key::WarnSexMismatch).to_string());
            }
        }
        // The same biomarker twice in one report (e.g. a test repeated by a
        // partner lab): keep the first, leave the rest for the user to pick.
        let mut seen = std::collections::HashSet::new();
        let mut repeated = Vec::new();
        let rows: Vec<(LabResult, bool)> = parsed
            .results
            .iter()
            .map(|r| {
                let first = seen.insert(r.biomarker.clone());
                if !first {
                    repeated.push(r.original_name.clone());
                }
                (r.clone(), first)
            })
            .collect();
        if !repeated.is_empty() {
            warnings.push(format!(
                "{}: {}",
                self.t(Key::WarnRepeated),
                repeated.join(", ")
            ));
        }
        let date = parsed
            .collected_on
            .unwrap_or_else(|| Local::now().date_naive());
        self.screen = Screen::Review(Review {
            file_name: path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            bytes,
            rows,
            parsed,
            date,
            selected: 0,
            warnings,
        });
        Ok(())
    }

    // --- review --------------------------------------------------------------------

    fn review_key(&mut self, key: KeyEvent) -> Result<()> {
        let Screen::Review(review) = &mut self.screen else {
            return Ok(());
        };
        let last = review.rows.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => self.screen = Screen::Dashboard,
            KeyCode::Up | KeyCode::Char('k') => review.selected = review.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => review.selected = (review.selected + 1).min(last),
            KeyCode::PageUp => review.selected = review.selected.saturating_sub(10),
            KeyCode::PageDown => review.selected = (review.selected + 10).min(last),
            KeyCode::Char(' ') => {
                if let Some(row) = review.rows.get_mut(review.selected) {
                    row.1 = !row.1;
                }
            }
            KeyCode::Char('a') => {
                let all = review.rows.iter().all(|r| r.1);
                review.rows.iter_mut().for_each(|r| r.1 = !all);
            }
            KeyCode::Char('e') => {
                if let Some((row, _)) = review.rows.get(review.selected) {
                    let current = row
                        .value
                        .map(|v| crate::units::format_number(v, self.lang.decimal_comma()))
                        .unwrap_or_default();
                    self.popup = Some(Popup::Input {
                        kind: InputKind::ReviewValue(review.selected),
                        input: TextInput::new(current),
                        error: false,
                    });
                }
            }
            KeyCode::Char('d') => {
                self.popup = Some(Popup::Input {
                    kind: InputKind::ReviewDate,
                    input: TextInput::new(review.date.format("%d/%m/%Y").to_string()),
                    error: false,
                });
            }
            KeyCode::Enter | KeyCode::Char('s') => {
                let results: Vec<LabResult> = review
                    .rows
                    .iter()
                    .filter(|(_, include)| *include)
                    .map(|(r, _)| r.clone())
                    .collect();
                if results.is_empty() {
                    return Ok(());
                }
                let review = match std::mem::replace(&mut self.screen, Screen::Dashboard) {
                    Screen::Review(r) => r,
                    _ => unreachable!(),
                };
                self.db.save_report(
                    &review.parsed,
                    review.date,
                    &results,
                    &review.bytes,
                    &review.file_name,
                )?;
                self.reload()?;
                self.tab = Tab::Biomarkers;
                self.info(format!(
                    "{}: {} {}",
                    self.t(Key::Saved),
                    results.len(),
                    self.t(Key::Results)
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets() {
        let r = |min, max| Some(Some(Range { min, max }));
        assert_eq!(parse_target("30-60"), r(Some(30.0), Some(60.0)));
        assert_eq!(parse_target("-100"), r(None, Some(100.0)));
        assert_eq!(parse_target("40-"), r(Some(40.0), None));
        assert_eq!(parse_target("1,5-2,5"), r(Some(1.5), Some(2.5)));
        assert_eq!(parse_target(""), Some(None));
        assert_eq!(parse_target("60-30"), None);
        assert_eq!(parse_target("abc"), None);
        assert_eq!(parse_target("-"), None);
    }

    #[test]
    fn validates_profile_form() {
        let today = NaiveDate::from_ymd_opt(2026, 10, 7).unwrap();
        let mut form = ProfileForm::new();
        assert!(form.validate(today).is_none());
        assert_eq!(form.errors[FIELD_NAME], Some(Key::ErrName));
        assert_eq!(form.focus, FIELD_NAME);

        form.name = TextInput::new("Ana");
        form.birth = TextInput::new("31/02/1990");
        form.weight = TextInput::new("62,5");
        form.height = TextInput::new("165");
        assert!(form.validate(today).is_none());
        assert_eq!(form.errors[FIELD_BIRTH], Some(Key::ErrDate));

        form.birth = TextInput::new("28/02/1990");
        let (p, w) = form.validate(today).unwrap();
        assert_eq!(p.name, "Ana");
        assert_eq!(w, 62.5);
        assert_eq!(p.height_cm, 165.0);
    }

    #[test]
    fn date_input_inserts_slashes() {
        let mut input = TextInput::default();
        for c in "25031992".chars() {
            input.handle_date(KeyEvent::from(KeyCode::Char(c)));
        }
        assert_eq!(input.value, "25/03/1992");
    }
}
