//! Layout-preserving text extraction from PDFs, in pure Rust.
//!
//! `pdf_extract::extract_text` drops spaces and line breaks for PDFs that place
//! every glyph individually (e.g. reports printed from a browser via cairo).
//! Instead we collect each glyph's position and rebuild lines and spacing
//! ourselves, similar to `pdftotext -layout`.

use anyhow::{Context, Result};
use pdf_extract::{Document, MediaBox, OutputDev, OutputError, Transform};

struct Glyph {
    page: u32,
    x: f64,
    /// Vertical position growing downwards (top of page = smallest).
    y: f64,
    end: f64,
    size: f64,
    text: String,
}

#[derive(Default)]
struct Collector {
    page: u32,
    glyphs: Vec<Glyph>,
}

impl OutputDev for Collector {
    fn begin_page(
        &mut self,
        page_num: u32,
        _media_box: &MediaBox,
        _art_box: Option<(f64, f64, f64, f64)>,
    ) -> Result<(), OutputError> {
        self.page = page_num;
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn output_character(
        &mut self,
        trm: &Transform,
        width: f64,
        _spacing: f64,
        font_size: f64,
        text: &str,
    ) -> Result<(), OutputError> {
        // Spacing is rebuilt from glyph positions. Explicit space glyphs only
        // hurt: some PDFs overlay space-padded runs on the same line (Unimed's
        // `Resultado:` + `     89 mg/dL`), interleaving spaces between letters.
        if text.trim().is_empty() {
            return Ok(());
        }
        let (vx, vy) = (
            font_size * (trm.m11 + trm.m21),
            font_size * (trm.m12 + trm.m22),
        );
        let size = (vx * vy).abs().sqrt();
        // A negative vertical scale means the page is drawn top-down (y grows
        // downwards); otherwise it uses regular PDF coordinates (y grows upwards).
        let y = if trm.m22 < 0.0 { trm.m32 } else { -trm.m32 };
        self.glyphs.push(Glyph {
            page: self.page,
            x: trm.m31,
            y,
            end: trm.m31 + width * size,
            size,
            text: text.to_string(),
        });
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }

    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

/// Extracts the text of every page of the PDF, one `String` per page, with
/// lines in reading order and horizontal spacing approximating the layout.
pub fn extract_pages(bytes: &[u8]) -> Result<Vec<String>> {
    let doc = Document::load_mem(strip_trailing_garbage(bytes))
        .context("não foi possível abrir o PDF")?;
    let mut collector = Collector::default();
    pdf_extract::output_doc(&doc, &mut collector)
        .map_err(|e| anyhow::anyhow!("falha ao ler o texto do PDF: {e:?}"))?;
    Ok(layout(collector.glyphs))
}

/// Some lab portals append their HTML page after the PDF (e.g. Unimed's
/// report viewer), which hides the cross-reference table from the parser.
/// Cuts everything after the last `%%EOF` marker.
fn strip_trailing_garbage(bytes: &[u8]) -> &[u8] {
    const EOF: &[u8] = b"%%EOF";
    match bytes.windows(EOF.len()).rposition(|w| w == EOF) {
        Some(i) => &bytes[..i + EOF.len()],
        None => bytes,
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    Some(values[values.len() / 2])
}

fn char_width(glyphs: &[&Glyph]) -> Option<f64> {
    median(
        glyphs
            .iter()
            .map(|g| g.end - g.x)
            .filter(|w| *w > 0.1)
            .collect(),
    )
}

fn layout(mut glyphs: Vec<Glyph>) -> Vec<String> {
    glyphs.sort_by(|a, b| a.page.cmp(&b.page).then(a.y.total_cmp(&b.y)));

    let mut pages: Vec<String> = Vec::new();
    let mut start = 0;
    while start < glyphs.len() {
        let page = glyphs[start].page;
        let end = glyphs[start..]
            .iter()
            .position(|g| g.page != page)
            .map_or(glyphs.len(), |p| start + p);
        pages.push(layout_page(&glyphs[start..end]));
        start = end;
    }
    pages
}

fn layout_page(glyphs: &[Glyph]) -> String {
    let all: Vec<&Glyph> = glyphs.iter().collect();
    let page_cw = char_width(&all).unwrap_or(1.0);
    let left = glyphs.iter().map(|g| g.x).fold(f64::INFINITY, f64::min);

    // Group glyphs (already sorted by y) into lines.
    let mut lines: Vec<Vec<&Glyph>> = Vec::new();
    for g in glyphs {
        match lines.last_mut() {
            Some(line) if (g.y - line[0].y).abs() < g.size.max(line[0].size) * 0.4 => line.push(g),
            _ => lines.push(vec![g]),
        }
    }

    let mut out = String::new();
    for mut line in lines {
        line.sort_by(|a, b| a.x.total_cmp(&b.x));
        let cw = char_width(&line).unwrap_or(page_cw);
        let mut text = " ".repeat(((line[0].x - left) / page_cw).round().max(0.0) as usize);
        let mut prev_end: Option<f64> = None;
        for g in line {
            if let Some(pe) = prev_end {
                let gap = g.x - pe;
                if gap > cw * 0.3 {
                    let n = ((gap / cw).round() as usize).max(1);
                    text.push_str(&" ".repeat(n));
                }
            }
            text.push_str(&g.text);
            prev_end = Some(g.end);
        }
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}
