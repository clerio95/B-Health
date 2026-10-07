//! Rendering of every screen.

use chrono::{Local, NaiveDate};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Axis, Block, Cell, Chart, Clear, Dataset, GraphType, Paragraph, Row, Table, TableState, Tabs,
    Wrap,
};

use crate::app::{
    App, FIELD_BIRTH, FIELD_COUNT, FIELD_HEIGHT, FIELD_NAME, FIELD_SEX, FIELD_WEIGHT, FileBrowser,
    InputKind, Popup, ProfileForm, Review, Screen, Tab,
};
use crate::catalog::{self, Panel};
use crate::i18n::{Key, Lang};
use crate::insights::{self, Series, Status};
use crate::model::Sex;
use crate::units::{Range, format_number};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;

fn status_color(s: Status) -> Color {
    match s {
        Status::Low => Color::LightBlue,
        Status::High => Color::LightRed,
        Status::Normal => Color::Green,
        Status::Unknown => Color::Gray,
    }
}

/// Missing a goal range (desirable/optimal) is shown milder than being
/// outside a normal range.
fn result_color(s: Status, goal: bool) -> Color {
    if goal && s.is_out() {
        Color::Yellow
    } else {
        status_color(s)
    }
}

fn status_symbol(s: Status) -> &'static str {
    match s {
        Status::Low => "▼",
        Status::High => "▲",
        Status::Normal => "●",
        Status::Unknown => "○",
    }
}

fn num(lang: Lang, v: f64) -> String {
    format_number(v, lang.decimal_comma())
}

fn date(lang: Lang, d: NaiveDate) -> String {
    d.format(lang.date_format()).to_string()
}

fn range_text(lang: Lang, r: &Range) -> String {
    match (r.min, r.max) {
        (Some(a), Some(b)) => format!("{} – {}", num(lang, a), num(lang, b)),
        (Some(a), None) => format!("≥ {}", num(lang, a)),
        (None, Some(b)) => format!("≤ {}", num(lang, b)),
        (None, None) => "—".into(),
    }
}

fn sparkline(values: &[f64]) -> String {
    const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let values = &values[values.len().saturating_sub(8)..];
    let (min, max) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    values
        .iter()
        .map(|&v| {
            if (max - min).abs() < f64::EPSILON {
                BARS[3]
            } else {
                BARS[(((v - min) / (max - min)) * 7.0).round() as usize]
            }
        })
        .collect()
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width.min(area.width))])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    area
}

pub fn draw(f: &mut Frame, app: &App) {
    match &app.screen {
        Screen::Onboarding(form) => draw_onboarding(f, app, form),
        Screen::Dashboard => draw_dashboard(f, app),
        Screen::Browser(b) => draw_browser(f, app, b),
        Screen::Review(r) => draw_review(f, app, r),
    }
    if let Some(popup) = &app.popup {
        draw_popup(f, app, popup);
    }
}

fn footer(f: &mut Frame, app: &App, area: Rect, keys: Key) {
    let line = match &app.status {
        Some((msg, true)) => Line::from(msg.as_str()).fg(Color::LightRed).bold(),
        Some((msg, false)) => Line::from(msg.as_str()).fg(Color::Green).bold(),
        None => Line::from(app.t(keys)).fg(MUTED),
    };
    f.render_widget(Paragraph::new(line), area);
}

// --- onboarding --------------------------------------------------------------------

fn draw_onboarding(f: &mut Frame, app: &App, form: &ProfileForm) {
    let lang = app.lang;
    let area = centered(f.area(), 70, 22);
    let title = if form.editing {
        app.t(Key::ProfileTitle)
    } else {
        app.t(Key::Welcome)
    };
    let block = Block::bordered()
        .title(format!(" B-Health · {title} "))
        .title_alignment(Alignment::Center)
        .border_style(Style::new().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(app.t(Key::WelcomeHint)).fg(MUTED),
        Line::default(),
    ];
    let sex_value = || {
        let opt = |s: Sex, key: Key| {
            let label = format!(" {} ", lang.t(key));
            if form.sex == s {
                Span::styled(label, Style::new().black().on_cyan().bold())
            } else {
                Span::raw(label)
            }
        };
        vec![
            opt(Sex::Male, Key::Male),
            Span::raw("  "),
            opt(Sex::Female, Key::Female),
        ]
    };
    for field in 0..FIELD_COUNT {
        let (label, value) = match field {
            FIELD_NAME => (Key::Name, form.name.value.as_str()),
            FIELD_BIRTH => (Key::BirthDate, form.birth.value.as_str()),
            FIELD_SEX => (Key::Sex, ""),
            FIELD_WEIGHT => (Key::Weight, form.weight.value.as_str()),
            FIELD_HEIGHT => (Key::Height, form.height.value.as_str()),
            _ => unreachable!(),
        };
        let focused = form.focus == field;
        let marker = if focused { "▶ " } else { "  " };
        let mut spans = vec![
            Span::styled(marker, Style::new().fg(ACCENT)),
            Span::styled(
                format!("{:<22}", lang.t(label)),
                if focused {
                    Style::new().fg(ACCENT).bold()
                } else {
                    Style::new()
                },
            ),
        ];
        if field == FIELD_SEX {
            spans.extend(sex_value());
        } else {
            let placeholder = match (field, lang) {
                (FIELD_BIRTH, _) => "DD/MM/AAAA",
                (FIELD_WEIGHT, Lang::Pt) => "ex.: 75,5",
                (FIELD_WEIGHT, Lang::En) => "e.g. 75.5",
                (FIELD_HEIGHT, Lang::Pt) => "ex.: 178",
                (FIELD_HEIGHT, Lang::En) => "e.g. 178",
                _ => "",
            };
            if value.is_empty() && !focused {
                spans.push(Span::styled(placeholder, Style::new().fg(MUTED)));
            } else {
                spans.push(Span::raw(value.to_string()));
            }
            if focused {
                spans.push(Span::styled("█", Style::new().fg(ACCENT)));
            }
        }
        lines.push(Line::from(spans));
        match form.errors[field] {
            Some(err) => lines.push(Line::from(format!("    {}", lang.t(err))).fg(Color::LightRed)),
            None => lines.push(Line::default()),
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(app.t(Key::OnboardingKeys)).fg(MUTED));
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

// --- dashboard ---------------------------------------------------------------------

fn draw_dashboard(f: &mut Frame, app: &App) {
    let [header, tabs, body, foot] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_header(f, app, header);
    let titles: Vec<String> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{} {}", i + 1, t.title(app.lang)))
        .collect();
    let selected = Tab::ALL.iter().position(|t| *t == app.tab).unwrap_or(0);
    f.render_widget(
        Tabs::new(titles)
            .select(selected)
            .highlight_style(Style::new().fg(ACCENT).bold().underlined())
            .divider("│"),
        tabs,
    );

    match app.tab {
        Tab::Biomarkers => draw_biomarkers(f, app, body),
        Tab::Insights => draw_insights(f, app, body),
        Tab::Reports => draw_reports(f, app, body),
    }
    let keys = if app.tab == Tab::Reports {
        Key::ReportsKeys
    } else {
        Key::DashboardKeys
    };
    footer(f, app, foot, keys);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let lang = app.lang;
    let mut spans = vec![Span::styled(
        " B-Health ",
        Style::new().black().on_cyan().bold(),
    )];
    if let Some(p) = &app.profile {
        let today = Local::now().date_naive();
        spans.push(Span::raw(format!(
            "  {} · {} {} · {} cm",
            p.name,
            p.age_on(today),
            lang.t(Key::Years),
            num(lang, p.height_cm)
        )));
        if let Some(w) = app.current_weight() {
            let bmi = insights::bmi(w, p.height_cm);
            spans.push(Span::raw(format!(" · {} kg · ", num(lang, w))));
            spans.push(Span::styled(
                format!(
                    "{} {} ({})",
                    lang.t(Key::Bmi),
                    num(lang, (bmi * 10.0).round() / 10.0),
                    lang.t(insights::bmi_class(bmi))
                ),
                Style::new().fg(if insights::bmi_class(bmi) == Key::BmiNormal {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn empty_state(f: &mut Frame, app: &App, area: Rect) {
    let text = Text::from(vec![
        Line::default(),
        Line::from(app.t(Key::NoData)).bold(),
        Line::from(app.t(Key::NoDataHint)).fg(ACCENT),
    ])
    .centered();
    f.render_widget(Paragraph::new(text).block(Block::bordered()), area);
}

fn draw_biomarkers(f: &mut Frame, app: &App, area: Rect) {
    if app.series.is_empty() {
        return empty_state(f, app, area);
    }
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).areas(area);
    draw_series_table(f, app, list);
    if let Some(s) = app.series.get(app.selected) {
        draw_detail(f, app, s, detail);
    }
}

fn draw_series_table(f: &mut Frame, app: &App, area: Rect) {
    let lang = app.lang;
    let mut rows = Vec::new();
    let mut selected_row = 0;
    let mut panel: Option<Panel> = None;
    for (i, s) in app.series.iter().enumerate() {
        if panel != Some(s.panel()) {
            panel = Some(s.panel());
            rows.push(
                Row::new(vec![Cell::from(s.panel().name(lang).to_uppercase())])
                    .style(Style::new().fg(ACCENT).bold()),
            );
        }
        if i == app.selected {
            selected_row = rows.len();
        }
        let latest = &s.latest().result;
        let status = s.status();
        let value = match (latest.value, &latest.text_value) {
            (Some(v), _) => format!("{} {}", num(lang, v), latest.unit),
            (None, Some(t)) => t.clone(),
            (None, None) => "—".into(),
        };
        let change = match s.change() {
            Some((_, pct)) => {
                let arrow = if pct > 0.5 {
                    "↑"
                } else if pct < -0.5 {
                    "↓"
                } else {
                    "→"
                };
                let style = if pct.abs() >= insights::BIG_CHANGE_PCT {
                    Style::new().fg(Color::Yellow).bold()
                } else {
                    Style::new().fg(MUTED)
                };
                let lab_mark = if s.lab_change().is_some() { "⇄" } else { "" };
                Cell::from(format!(
                    "{arrow}{}%{lab_mark}",
                    num(lang, pct.abs().round())
                ))
                .style(style)
            }
            None => Cell::from(""),
        };
        let values: Vec<f64> = s.values().iter().map(|(_, v)| *v).collect();
        let target_mark = match s.target_status() {
            Some(t) if t.is_out() => Span::styled(" ◆", Style::new().fg(Color::Yellow)),
            Some(_) => Span::styled(" ◆", Style::new().fg(Color::Green)),
            None => Span::raw(""),
        };
        rows.push(Row::new(vec![
            Cell::from(format!("  {}", s.name(lang))),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!("{} ", status_symbol(status)),
                    Style::new().fg(result_color(status, s.goal_range())),
                ),
                Span::raw(value),
                target_mark,
            ])),
            change,
            Cell::from(if values.len() > 1 {
                sparkline(&values)
            } else {
                String::new()
            })
            .style(Style::new().fg(ACCENT)),
        ]));
    }
    let table = Table::new(
        rows,
        [
            Constraint::Fill(3),
            Constraint::Fill(3),
            Constraint::Length(9),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec![
            app.t(Key::TabBiomarkers),
            app.t(Key::LastValue),
            app.t(Key::Change),
            "",
        ])
        .style(Style::new().fg(MUTED)),
    )
    .row_highlight_style(Style::new().bg(Color::Rgb(40, 50, 60)).bold())
    .block(Block::bordered());
    let mut state = TableState::default().with_selected(Some(selected_row));
    f.render_stateful_widget(table, area, &mut state);
}

fn draw_detail(f: &mut Frame, app: &App, s: &Series, area: Rect) {
    let lang = app.lang;
    let block = Block::bordered()
        .title(format!(" {} ", s.name(lang)))
        .title_style(Style::new().fg(ACCENT).bold());
    let inner = block.inner(area);
    f.render_widget(block, area);

    let latest = &s.latest().result;
    let status = s.status();
    let mut lines = Vec::new();
    let value = match (latest.value, &latest.text_value) {
        (Some(v), _) => format!("{} {}", num(lang, v), latest.unit),
        (None, Some(t)) => t.clone(),
        (None, None) => "—".into(),
    };
    lines.push(Line::from(vec![
        Span::styled(value, Style::new().bold()),
        Span::raw("  "),
        Span::styled(
            format!(
                "{} {}",
                status_symbol(status),
                status.goal_label(s.goal_range(), lang)
            ),
            Style::new().fg(result_color(status, s.goal_range())).bold(),
        ),
        Span::styled(
            format!("  ({})", date(lang, s.latest().date)),
            Style::new().fg(MUTED),
        ),
    ]));
    if let Some((before, now)) = s.lab_change() {
        lines.push(
            Line::from(format!("⇄ {before} → {now}: {}", app.t(Key::LabChanged))).fg(Color::Yellow),
        );
    }
    if !latest.reference.is_empty() || !latest.reference_text.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", app.t(Key::Reference)),
                Style::new().fg(MUTED),
            ),
            Span::raw(range_text(lang, &latest.reference)),
            Span::styled(
                if s.goal_range() {
                    format!(" ({})", app.t(Key::Desirable))
                } else {
                    String::new()
                },
                Style::new().fg(MUTED),
            ),
        ]));
        let ref_lines: Vec<&str> = latest.reference_text.lines().collect();
        if ref_lines.len() > 1 {
            for l in ref_lines {
                lines.push(Line::from(format!("  {}", l.trim())).fg(MUTED));
            }
        }
    }
    if let Some(t) = s.target {
        let ts = s.target_status().unwrap_or(Status::Unknown);
        lines.push(Line::from(vec![
            Span::styled(format!("{}: ", app.t(Key::Target)), Style::new().fg(MUTED)),
            Span::raw(range_text(lang, &t)),
            Span::styled(
                format!("  {}", status_symbol(ts)),
                Style::new().fg(status_color(ts)),
            ),
        ]));
    }
    if let Some(def) = s.def {
        lines.push(Line::default());
        let mut note = |label: Key, text: (&'static str, &'static str), highlight: bool| {
            let text = catalog::pick(lang, text);
            if text.is_empty() {
                return;
            }
            let style = if highlight {
                Style::new().fg(Color::Yellow)
            } else {
                Style::new()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}: ", lang.t(label)), Style::new().fg(MUTED)),
                Span::styled(text, style),
            ]));
        };
        note(Key::About, def.about, false);
        note(Key::IfHigh, def.high, status == Status::High);
        note(Key::IfLow, def.low, status == Status::Low);
    } else if latest.original_name != s.name(lang) {
        lines.push(Line::from(latest.original_name.clone()).fg(MUTED));
    }

    let values = s.values();
    let chart_height = if values.len() >= 2 { 12 } else { 0 };
    let [info, chart, history] = Layout::vertical([
        Constraint::Min(4),
        Constraint::Length(chart_height),
        Constraint::Length((s.points.len() as u16 + 2).min(10)),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), info);
    if values.len() >= 2 {
        draw_chart(f, lang, s, &values, chart);
    }

    let rows = s.points.iter().rev().map(|m| {
        let st = Status::of(m.result.value, &m.result.reference);
        let goal = insights::is_goal_range(&m.result.reference_text);
        let v = match (m.result.value, &m.result.text_value) {
            (Some(v), _) => num(lang, v),
            (None, Some(t)) => t.clone(),
            _ => "—".into(),
        };
        Row::new(vec![
            Cell::from(date(lang, m.date)),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!("{} ", status_symbol(st)),
                    Style::new().fg(result_color(st, goal)),
                ),
                Span::raw(v),
            ])),
            Cell::from(m.lab.clone()),
            Cell::from(range_text(lang, &m.result.reference)).style(Style::new().fg(MUTED)),
        ])
    });
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(24),
                Constraint::Fill(1),
            ],
        )
        .header(
            Row::new(vec![
                app.t(Key::Date),
                app.t(Key::Value),
                app.t(Key::Lab),
                app.t(Key::Reference),
            ])
            .style(Style::new().fg(MUTED)),
        )
        .block(Block::new().title(format!("─ {} ", app.t(Key::History)))),
        history,
    );
}

fn draw_chart(f: &mut Frame, lang: Lang, s: &Series, values: &[(NaiveDate, f64)], area: Rect) {
    let first = values[0].0;
    let x = |d: NaiveDate| (d - first).num_days() as f64;
    let points: Vec<(f64, f64)> = values.iter().map(|(d, v)| (x(*d), *v)).collect();
    let x_max = points.last().map_or(1.0, |p| p.0).max(1.0);
    let reference = s.latest().result.reference;

    let mut lo = values.iter().map(|v| v.1).fold(f64::INFINITY, f64::min);
    let mut hi = values.iter().map(|v| v.1).fold(f64::NEG_INFINITY, f64::max);
    for bound in [reference.min, reference.max].into_iter().flatten() {
        lo = lo.min(bound);
        hi = hi.max(bound);
    }
    let pad = ((hi - lo) * 0.1).max(hi.abs() * 0.02).max(0.01);
    // Lab values are never negative; don't pad the axis below zero.
    let (lo, hi) = (
        if lo >= 0.0 {
            (lo - pad).max(0.0)
        } else {
            lo - pad
        },
        hi + pad,
    );

    let min_line: Vec<(f64, f64)> = reference
        .min
        .map(|m| vec![(0.0, m), (x_max, m)])
        .unwrap_or_default();
    let max_line: Vec<(f64, f64)> = reference
        .max
        .map(|m| vec![(0.0, m), (x_max, m)])
        .unwrap_or_default();

    let mut datasets = vec![];
    if !min_line.is_empty() {
        datasets.push(
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(
                    Style::new()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::DIM),
                )
                .data(&min_line),
        );
    }
    if !max_line.is_empty() {
        datasets.push(
            Dataset::default()
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(Color::LightRed).add_modifier(Modifier::DIM))
                .data(&max_line),
        );
    }
    datasets.push(
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::new().fg(ACCENT))
            .data(&points),
    );
    datasets.push(
        Dataset::default()
            .marker(Marker::Dot)
            .graph_type(GraphType::Scatter)
            .style(Style::new().fg(Color::White).bold())
            .data(&points),
    );
    let chart = Chart::new(datasets)
        .x_axis(
            Axis::default()
                .bounds([0.0, x_max])
                .labels([
                    Span::raw(date(lang, first)),
                    Span::raw(date(lang, values.last().unwrap().0)),
                ])
                .style(Style::new().fg(MUTED)),
        )
        .y_axis(
            Axis::default()
                .bounds([lo, hi])
                .labels([Span::raw(num(lang, lo)), Span::raw(num(lang, hi))])
                .style(Style::new().fg(MUTED)),
        );
    f.render_widget(chart, area);
}

fn draw_insights(f: &mut Frame, app: &App, area: Rect) {
    let lang = app.lang;
    let Some(profile) = &app.profile else {
        return;
    };
    let mut lines: Vec<Line> = Vec::new();
    let heading = |text: &str| Line::from(text.to_uppercase()).fg(ACCENT).bold();

    // Out of range in the latest measurement of each biomarker.
    lines.push(heading(app.t(Key::OutOfRange)));
    let out: Vec<&Series> = app.series.iter().filter(|s| s.status().is_out()).collect();
    if app.series.is_empty() {
        lines.push(Line::from(app.t(Key::NoDataHint)).fg(MUTED));
    } else if out.is_empty() {
        lines.push(Line::from(app.t(Key::AllNormal)).fg(Color::Green));
    }
    for s in out {
        let st = s.status();
        let r = &s.latest().result;
        let goal = s.goal_range();
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", status_symbol(st)),
                Style::new().fg(result_color(st, goal)),
            ),
            Span::styled(format!("{:<28}", s.name(lang)), Style::new().bold()),
            Span::raw(format!(
                "{} {}",
                r.value.map(|v| num(lang, v)).unwrap_or_default(),
                r.unit
            )),
            Span::styled(
                format!(
                    "  [{}{}]  {}",
                    if goal {
                        format!("{} ", app.t(Key::Desirable))
                    } else {
                        String::new()
                    },
                    range_text(lang, &r.reference),
                    date(lang, s.latest().date)
                ),
                Style::new().fg(MUTED),
            ),
        ]));
        if let Some(def) = s.def {
            let note = catalog::pick(
                lang,
                if st == Status::High {
                    def.high
                } else {
                    def.low
                },
            );
            if !note.is_empty() {
                lines.push(Line::from(format!("     {note}")).fg(Color::Yellow));
            }
        }
    }
    lines.push(Line::default());

    lines.push(heading(&format!(
        "{} (≥{}%)",
        app.t(Key::BigChanges),
        insights::BIG_CHANGE_PCT
    )));
    let changes = insights::big_changes(&app.series);
    if changes.is_empty() {
        lines.push(Line::from("—").fg(MUTED));
    }
    let any_lab_change = changes.iter().any(|(s, _)| s.lab_change().is_some());
    for (s, pct) in changes {
        let prev = s.previous().expect("change implies previous");
        let lab_change = s.lab_change();
        // Softer (muted) arrow when the labs differ: the change may be partly
        // methodological.
        let arrow_color = if lab_change.is_some() {
            Color::Gray
        } else {
            Color::Yellow
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", if pct > 0.0 { "↑" } else { "↓" }),
                Style::new().fg(arrow_color),
            ),
            Span::styled(format!("{:<28}", s.name(lang)), Style::new().bold()),
            Span::raw(format!(
                "{} → {} {}",
                prev.result.value.map(|v| num(lang, v)).unwrap_or_default(),
                s.latest()
                    .result
                    .value
                    .map(|v| num(lang, v))
                    .unwrap_or_default(),
                s.latest().result.unit
            )),
            Span::styled(
                format!(
                    "  ({}{}%, {} → {})",
                    if pct > 0.0 { "+" } else { "−" },
                    num(lang, pct.abs().round()),
                    date(lang, prev.date),
                    date(lang, s.latest().date)
                ),
                Style::new().fg(MUTED),
            ),
            Span::styled(
                lab_change
                    .map(|(before, now)| format!("  ⇄ {before} → {now}"))
                    .unwrap_or_default(),
                Style::new().fg(Color::Yellow),
            ),
        ]));
    }
    if any_lab_change {
        lines.push(Line::from(format!("   ⇄ {}", app.t(Key::LabChanged))).fg(MUTED));
    }
    lines.push(Line::default());

    lines.push(heading(app.t(Key::Derived)));
    for d in insights::derived(&app.series, profile, &app.weights) {
        let value = (d.value * 100.0).round() / 100.0;
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", status_symbol(d.status)),
                Style::new().fg(status_color(d.status)),
            ),
            Span::styled(
                format!("{:<28}", catalog::pick(lang, d.name)),
                Style::new().bold(),
            ),
            Span::raw(format!("{} {}", num(lang, value), d.unit)),
            Span::styled(format!("  {}", date(lang, d.date)), Style::new().fg(MUTED)),
        ]));
        lines.push(Line::from(format!("     {}", lang.t(d.note))).fg(MUTED));
    }
    lines.push(Line::default());
    lines.push(Line::from(app.t(Key::Disclaimer)).italic().fg(MUTED));

    f.render_widget(
        Paragraph::new(lines)
            .block(Block::bordered())
            .wrap(Wrap { trim: false })
            .scroll((app.insights_scroll, 0)),
        area,
    );
}

fn draw_reports(f: &mut Frame, app: &App, area: Rect) {
    if app.reports.is_empty() {
        return empty_state(f, app, area);
    }
    let lang = app.lang;
    let rows = app.reports.iter().map(|(r, count)| {
        Row::new(vec![
            date(lang, r.collected_on),
            r.lab.clone(),
            r.order_no.clone().unwrap_or_default(),
            count.to_string(),
            r.file_name.clone(),
        ])
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(12),
            Constraint::Length(26),
            Constraint::Length(18),
            Constraint::Length(6),
            Constraint::Fill(1),
        ],
    )
    .header(
        Row::new(vec![
            app.t(Key::Collected),
            app.t(Key::Lab),
            "OS",
            "#",
            "PDF",
        ])
        .style(Style::new().fg(MUTED)),
    )
    .row_highlight_style(Style::new().bg(Color::Rgb(40, 50, 60)).bold())
    .block(Block::bordered());
    let mut state = TableState::default().with_selected(Some(app.report_selected));
    f.render_stateful_widget(table, area, &mut state);
}

// --- file browser ----------------------------------------------------------------------

fn draw_browser(f: &mut Frame, app: &App, b: &FileBrowser) {
    let [body, foot] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(f.area());
    let rows = b.entries.iter().map(|(name, is_dir)| {
        if *is_dir {
            Row::new(vec![format!("📁 {name}/")]).style(Style::new().fg(ACCENT))
        } else {
            Row::new(vec![format!("📄 {name}")])
        }
    });
    let table = Table::new(rows, [Constraint::Fill(1)])
        .row_highlight_style(Style::new().bg(Color::Rgb(40, 50, 60)).bold())
        .block(
            Block::bordered()
                .title(format!(" {} ", app.t(Key::PickFile)))
                .title_bottom(format!(" {} ", b.dir.display()))
                .title_style(Style::new().fg(ACCENT).bold()),
        );
    let mut state = TableState::default().with_selected(Some(b.selected));
    f.render_stateful_widget(table, body, &mut state);
    footer(f, app, foot, Key::PickFileKeys);
}

// --- review --------------------------------------------------------------------------------

fn draw_review(f: &mut Frame, app: &App, r: &Review) {
    let lang = app.lang;
    let warn_height = r.warnings.len() as u16;
    let [info, warnings, body, foot] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(warn_height),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let included = r.rows.iter().filter(|x| x.1).count();
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", app.t(Key::Review)),
                    Style::new().black().on_cyan().bold(),
                ),
                Span::raw(format!("  {}", r.file_name)),
            ]),
            Line::from(vec![
                Span::styled(format!(" {}: ", app.t(Key::Lab)), Style::new().fg(MUTED)),
                Span::raw(r.parsed.lab.clone()),
                Span::styled(
                    format!("   {}: ", app.t(Key::Collected)),
                    Style::new().fg(MUTED),
                ),
                Span::styled(date(lang, r.date), Style::new().bold()),
                Span::styled(
                    format!("   {included}/{} {}", r.rows.len(), app.t(Key::Results)),
                    Style::new().fg(MUTED),
                ),
            ]),
        ]),
        info,
    );
    f.render_widget(
        Paragraph::new(
            r.warnings
                .iter()
                .map(|w| Line::from(format!(" ⚠ {w}")).fg(Color::Yellow))
                .collect::<Vec<_>>(),
        ),
        warnings,
    );

    let rows = r.rows.iter().map(|(res, include)| {
        let name = catalog::get(&res.biomarker)
            .map(|b| b.name(lang).to_string())
            .unwrap_or_else(|| format!("? {}", res.original_name));
        let st = Status::of(res.value, &res.reference);
        let goal = insights::is_goal_range(&res.reference_text);
        let value = match (res.value, &res.text_value) {
            (Some(v), _) => num(lang, v),
            (None, Some(t)) => t.clone(),
            _ => "—".into(),
        };
        let style = if *include {
            Style::new()
        } else {
            Style::new().fg(MUTED).crossed_out()
        };
        Row::new(vec![
            Cell::from(if *include { "[x]" } else { "[ ]" }),
            Cell::from(name),
            Cell::from(res.original_name.clone()).style(Style::new().fg(MUTED)),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!("{} ", status_symbol(st)),
                    Style::new().fg(result_color(st, goal)),
                ),
                Span::raw(value),
            ])),
            Cell::from(res.unit.clone()),
            Cell::from(range_text(lang, &res.reference)),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Fill(3),
            Constraint::Fill(3),
            Constraint::Length(14),
            Constraint::Length(13),
            Constraint::Fill(2),
        ],
    )
    .header(
        Row::new(vec![
            app.t(Key::Include),
            app.t(Key::TabBiomarkers),
            app.t(Key::OriginalName),
            app.t(Key::Value),
            app.t(Key::Unit),
            app.t(Key::Reference),
        ])
        .style(Style::new().fg(MUTED)),
    )
    .row_highlight_style(Style::new().bg(Color::Rgb(40, 50, 60)).bold())
    .block(Block::bordered());
    let mut state = TableState::default().with_selected(Some(r.selected));
    f.render_stateful_widget(table, body, &mut state);
    footer(f, app, foot, Key::ReviewKeys);
}

// --- popups ------------------------------------------------------------------------------------

fn draw_popup(f: &mut Frame, app: &App, popup: &Popup) {
    let area = centered(f.area(), 60, 5);
    f.render_widget(Clear, area);
    match popup {
        Popup::ConfirmDelete(_) => {
            f.render_widget(
                Paragraph::new(app.t(Key::ConfirmDelete))
                    .wrap(Wrap { trim: true })
                    .block(Block::bordered().border_style(Style::new().fg(Color::LightRed))),
                area,
            );
        }
        Popup::Input { kind, input, error } => {
            let title = match kind {
                InputKind::Weight => app.t(Key::WeightPrompt),
                InputKind::Target(_) => app.t(Key::TargetPrompt),
                InputKind::ReviewValue(_) => app.t(Key::EditValue),
                InputKind::ReviewDate => app.t(Key::EditDate),
            };
            let border = if *error { Color::LightRed } else { ACCENT };
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::raw(input.value.clone()),
                    Span::styled("█", Style::new().fg(ACCENT)),
                ]))
                .block(
                    Block::bordered()
                        .title(format!(" {title} "))
                        .title_bottom(" Enter ✓  Esc ✗ ")
                        .border_style(Style::new().fg(border)),
                ),
                area,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_scales_values() {
        assert_eq!(sparkline(&[1.0, 2.0, 3.0]), "▁▅█");
        assert_eq!(sparkline(&[5.0, 5.0]), "▄▄");
    }

    #[test]
    fn formats_ranges() {
        let r = |min, max| Range { min, max };
        assert_eq!(range_text(Lang::Pt, &r(Some(60.0), Some(99.0))), "60 – 99");
        assert_eq!(range_text(Lang::Pt, &r(None, Some(5.7))), "≤ 5,7");
        assert_eq!(range_text(Lang::En, &r(Some(3.37), None)), "≥ 3.37");
        assert_eq!(range_text(Lang::En, &r(None, None)), "—");
    }
}

/// End-to-end smoke test over the real sample PDFs (kept out of git):
/// `cargo test screens -- --ignored --nocapture` prints every screen.
#[cfg(test)]
mod screens {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent};

    use crate::app::{App, Screen};
    use crate::db::Db;

    fn render(app: &App, title: &str) {
        let mut term = Terminal::new(TestBackend::new(150, 42)).unwrap();
        term.draw(|f| super::draw(f, app)).unwrap();
        println!("===== {title} =====");
        let buf = term.backend().buffer();
        for y in 0..buf.area.height {
            let line: String = (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect();
            println!("{}", line.trim_end());
        }
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::from(code));
    }

    fn type_text(app: &mut App, text: &str) {
        text.chars().for_each(|c| press(app, KeyCode::Char(c)));
    }

    #[test]
    #[ignore = "needs samples/*.pdf"]
    fn screens() {
        let dir = std::env::temp_dir().join(format!("b-health-screens-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut app = App::new(Db::open(&dir).unwrap(), None).unwrap();
        render(&app, "onboarding (empty)");
        press(&mut app, KeyCode::Enter);
        render(&app, "onboarding (validation)");
        type_text(&mut app, "Maria Teste");
        press(&mut app, KeyCode::Tab);
        type_text(&mut app, "25031992");
        press(&mut app, KeyCode::Tab);
        press(&mut app, KeyCode::Tab);
        type_text(&mut app, "78,5");
        press(&mut app, KeyCode::Tab);
        type_text(&mut app, "178");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.screen, Screen::Dashboard));
        render(&app, "dashboard (empty)");

        for pdf in [
            "samples/santamaria2.pdf",
            "samples/santamaria1.pdf",
            "samples/UNIMED1.pdf",
        ] {
            app.start_import(std::path::Path::new(pdf));
            assert!(matches!(app.screen, Screen::Review(_)), "{:?}", app.status);
            render(&app, &format!("review {pdf}"));
            press(&mut app, KeyCode::Enter);
        }
        app.start_import(std::path::Path::new("samples/santamaria1.pdf"));
        assert!(
            app.status.as_ref().is_some_and(|s| s.1),
            "duplicate must be rejected"
        );

        for _ in 0..17 {
            press(&mut app, KeyCode::Down);
        }
        render(&app, "biomarkers (monocytes %)");
        press(&mut app, KeyCode::Char('2'));
        render(&app, "insights");
        for _ in 0..30 {
            press(&mut app, KeyCode::Down);
        }
        render(&app, "insights (scrolled)");
        press(&mut app, KeyCode::Char('L'));
        render(&app, "insights (english)");
        press(&mut app, KeyCode::Char('3'));
        render(&app, "reports (english)");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
