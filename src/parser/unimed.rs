//! Unimed lab reports (as exported from Unimed's web report viewer).
//!
//! Unimed pages show results on the left and reference text in a right-hand
//! column:
//! - `Resultado:   89 mg/dL        De 74 a 106 mg/dL` (named by the exam title);
//! - `HBA1C:   4.78 %   Normal: Inferior a 5,7%` (`Name: value`);
//! - blood count rows `ERITRÓCITOS  5,49 milhões/mm³   4,50 a 5,50 milhões/mm³`.
//!
//! Tests outsourced to a partner lab (Álvaro/Dasa) are appended in another
//! layout: an `Exame: Resultado Valor(es) de referência:` header, several
//! results, and one shared reference column split by analyte headings.

use chrono::NaiveDate;

use super::common::{collapse_spaces, first_date, is_upper_words, parse_row};
use super::{LabParser, ParsedReport};
use crate::catalog::normalize;
use crate::model::{LabResult, Sex};
#[cfg(test)]
use crate::units::Range;
use crate::units::parse_br_number;

pub struct Unimed;

impl LabParser for Unimed {
    fn lab_name(&self) -> &'static str {
        "Unimed"
    }

    fn detect(&self, text: &str) -> bool {
        text.contains("Dt.Nasc./Sexo") && text.contains("Hash laudo")
    }

    fn parse(&self, pages: &[String]) -> ParsedReport {
        let mut p = Parser::default();
        for page in pages {
            for line in page.lines() {
                p.line(line);
            }
        }
        p.flush_partner();
        p.report
    }
}

/// Lines starting a reference column continuation are indented at least this
/// far; results and titles start near the left margin.
const REF_COLUMN: usize = 30;

/// Lines that start notes, signatures or method details (never results).
const NOTES: &[&str] = &[
    "NOTA",
    "Na ausência",
    "A Associação",
    "Conforme",
    "estamos",
    "uma fórmula",
    "American",
    "Bibliografia",
    "Observação",
    "Comentários",
    "Referências",
    "Locais de execução",
    "*OS resultados",
    "interpretação",
    "Hash laudo",
    "Triado",
    "Liberado",
    "Responsável",
    "Sob a responsabilidade",
    "Método",
    "Ensaio",
    "Os intervalos",
];

const HEADER: &[&str] = &[
    "Solicitante:",
    "Origem:",
    "Idade:",
    "Pág.",
    "Data de recebimento",
];

#[derive(Default)]
struct Parser {
    report: ParsedReport,
    /// Last title-like line (candidate exam name).
    title: String,
    exam: String,
    in_notes: bool,
    /// Inside a partner-lab page header (until `Exame:`).
    in_partner_header: bool,
    /// Current page uses the partner-lab layout.
    partner: bool,
    /// Unimed layout: result whose reference column is being read.
    ref_owner: Option<usize>,
    /// Partner layout: results of the current exam and their shared column.
    partner_results: Vec<usize>,
    partner_column: Vec<String>,
}

impl Parser {
    fn line(&mut self, raw: &str) {
        let t = raw.trim();
        if t.is_empty() {
            return;
        }
        let indent = raw.len() - raw.trim_start().len();

        // --- page headers -------------------------------------------------------
        if let Some(rest) = t.strip_prefix("Paciente:") {
            self.end_page_section();
            self.partner = false;
            self.in_partner_header = false;
            if self.report.patient_name.is_none() {
                let name = rest.split_once(" - ").map_or(rest, |(_, n)| n);
                self.report.patient_name = Some(collapse_spaces(name));
            }
            return;
        }
        if t.starts_with("Paciente") && t.contains("Sexo") {
            self.end_page_section();
            self.in_partner_header = true;
            return;
        }
        if t.starts_with("Exame:") {
            self.end_page_section();
            self.in_partner_header = false;
            self.partner = true;
            return;
        }
        if self.in_partner_header {
            return;
        }
        if let Some(rest) = t.strip_prefix("Pedido:") {
            if self.report.order_no.is_none() {
                self.report.order_no = rest.split_whitespace().next().map(str::to_string);
            }
            if let Some(cadastro) = rest.split("Cadastro:").nth(1) {
                self.collected(first_date(cadastro));
            }
            return;
        }
        if let Some(rest) = t.strip_prefix("Dt.Nasc./Sexo:") {
            self.header_birth(rest);
            return;
        }
        if let Some(rest) = t.strip_prefix("Data de coleta:") {
            self.exam = self.title.clone();
            self.collected(first_date(rest));
            return;
        }
        if HEADER.iter().any(|h| t.starts_with(h)) {
            return;
        }
        if t.starts_with("Material") {
            self.ref_owner = None;
            self.in_notes = false;
            if !self.partner {
                self.exam = self.title.clone();
            }
            return;
        }
        if NOTES.iter().any(|n| t.starts_with(n)) {
            if t.starts_with("Observação") || t.starts_with("Comentários") {
                self.flush_partner();
            }
            self.ref_owner = None;
            self.in_notes = true;
            return;
        }
        if t.contains("Valor de refer") || t.contains("Valores de Refer") {
            return;
        }

        // --- reference column continuation ---------------------------------
        if indent >= REF_COLUMN {
            if self.in_notes {
                return;
            }
            if self.partner {
                if !self.partner_results.is_empty() {
                    self.partner_column.push(t.to_string());
                }
            } else if let Some(i) = self.ref_owner {
                let r = &mut self.report.results[i];
                r.reference_text.push('\n');
                r.reference_text.push_str(t);
            }
            return;
        }

        // --- results -------------------------------------------------------------
        if let Some(rest) = t.strip_prefix("Resultado:") {
            self.in_notes = false;
            let name = self.exam.clone();
            self.result(&name, rest);
            return;
        }
        if let Some((name, rest)) = t.split_once(':')
            && rest.trim_start().starts_with(|c: char| c.is_ascii_digit())
            && self.result(name, rest)
        {
            return;
        }
        if let Some(num_at) = t
            .split_whitespace()
            .position(|tok| parse_br_number(tok).is_some())
            && num_at > 0
        {
            let mut tokens = t.split_whitespace();
            let name = tokens.by_ref().take(num_at).collect::<Vec<_>>().join(" ");
            let offset = t.find(tokens.next().unwrap_or("")).unwrap_or(t.len());
            if !name.contains(':') && !self.in_notes && self.result(&name, &t[offset..]) {
                return;
            }
        }

        // Partner layout: a result name wrapped onto a second line, beside
        // the shared reference column (`Sexuais (SHBG)    7 a 13 anos: ...`).
        if self.partner && !self.in_notes {
            let (left, right) = split_columns(t);
            if let Some(&last) = self.partner_results.last()
                && !left.chars().any(|c| c.is_ascii_digit())
                && !right.is_empty()
            {
                let r = &mut self.report.results[last];
                r.original_name = format!("{} {}", r.original_name, left);
                self.partner_column.push(right.to_string());
                return;
            }
        }

        if is_upper_words(t) && !t.contains(':') {
            if !t.starts_with("SÉRIE") {
                self.title = collapse_spaces(t);
                self.exam = self.title.clone();
            }
            self.in_notes = false;
        }
        self.ref_owner = None;
    }

    fn collected(&mut self, date: Option<NaiveDate>) {
        if let Some(date) = date
            && self.report.collected_on.is_none_or(|d| date < d)
        {
            self.report.collected_on = Some(date);
        }
    }

    fn header_birth(&mut self, rest: &str) {
        let mut parts = rest.split_whitespace();
        if self.report.birth_date.is_none()
            && let Some(date) = parts.next()
        {
            // The report viewer prints US-style dates here (`3/25/1992/`).
            let date = date.trim_end_matches('/');
            self.report.birth_date = NaiveDate::parse_from_str(date, "%m/%d/%Y")
                .or_else(|_| NaiveDate::parse_from_str(date, "%d/%m/%Y"))
                .ok();
        }
        if self.report.sex.is_none() {
            self.report.sex = match parts.next().map(normalize).as_deref() {
                Some("MASCULINO" | "M") => Some(Sex::Male),
                Some("FEMININO" | "F") => Some(Sex::Female),
                _ => None,
            };
        }
    }

    /// Records the results in `rest` (`value unit [ranges] [reference text]`).
    /// Returns false if `rest` has no value.
    fn result(&mut self, name: &str, rest: &str) -> bool {
        let name = collapse_spaces(name.trim_end_matches([':', '.']));
        if name.is_empty() || !name.chars().any(char::is_alphabetic) {
            return false;
        }
        let Some(row) = parse_row(rest) else {
            return false;
        };
        self.in_notes = false;
        let first = self.report.results.len();
        for (idx, (value, unit)) in row.values.into_iter().enumerate() {
            let (reference, text) = row.ranges.get(idx).cloned().unwrap_or_default();
            self.report.results.push(LabResult {
                biomarker: String::new(),
                original_name: name.clone(),
                exam: self.exam.clone(),
                value: Some(value),
                text_value: None,
                unit,
                reference,
                reference_text: text,
            });
        }
        self.ref_owner = None;
        if self.partner {
            self.partner_results.push(first);
            if !row.rest.is_empty() {
                self.partner_column.push(row.rest);
            }
        } else if !row.rest.is_empty() {
            let r = &mut self.report.results[first];
            if !r.reference_text.is_empty() {
                r.reference_text.push('\n');
            }
            r.reference_text.push_str(&row.rest);
            self.ref_owner = Some(first);
        }
        true
    }

    fn end_page_section(&mut self) {
        self.flush_partner();
        self.ref_owner = None;
        self.in_notes = false;
    }

    /// Splits the shared reference column of a partner-lab exam among its
    /// results, using lines that repeat a result's name as headings.
    fn flush_partner(&mut self) {
        let results = std::mem::take(&mut self.partner_results);
        let column = std::mem::take(&mut self.partner_column);
        if results.is_empty() || column.is_empty() {
            return;
        }
        let headings: Vec<Vec<String>> = results
            .iter()
            .map(|&i| heading_names(&self.report.results[i].original_name))
            .collect();
        let mut owner = 0;
        let mut blocks: Vec<Vec<String>> = vec![Vec::new(); results.len()];
        for line in column {
            let norm = normalize(&line);
            if let Some(j) = headings.iter().position(|h| h.contains(&norm)) {
                owner = j;
                if results.len() > 1 {
                    continue;
                }
            }
            blocks[owner].push(line);
        }
        for (&i, block) in results.iter().zip(blocks) {
            let r = &mut self.report.results[i];
            let mut text: Vec<String> = Vec::new();
            if !r.reference_text.is_empty() {
                text.push(std::mem::take(&mut r.reference_text));
            }
            text.extend(block);
            r.reference_text = text.join("\n");
        }
    }
}

/// A result's own name plus any parenthesized alias, normalized:
/// `Globulina ... Sexuais (SHBG)` → [`GLOBULINA ... SEXUAIS SHBG`, `SHBG`].
fn heading_names(name: &str) -> Vec<String> {
    let mut names = vec![normalize(name)];
    if let (Some(open), Some(close)) = (name.find('('), name.rfind(')'))
        && open < close
    {
        names.push(normalize(&name[open + 1..close]));
        names.push(normalize(&name[..open]));
    }
    names
}

/// Splits a line at its first run of 3+ spaces.
fn split_columns(t: &str) -> (&str, &str) {
    match t.find("   ") {
        Some(i) => (t[..i].trim(), t[i..].trim()),
        None => (t, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Patient, parse_pages};

    /// Synthetic excerpt in the layout of Unimed's exported PDFs.
    const PAGE1: &str = "\
  Paciente:      0000001 - JOAO DA SILVA TESTE
  Pedido:        10000001                                         Cadastro:      12/12/2025
  Solicitante:   DRA. FULANA CRM-ES-0000                          Emissão:       07/10/2026 14:28
  Dt.Nasc./Sexo: 3/25/1992/ MASCULINO
  Idade:         30a
                                                                                   Pág. 1 de 3
HEMOGRAMA COMPLETO
SÉRIE VERMELHA                                     Valores de Referência
ERITRÓCITOS             5,49 milhões/mm³                4,50 a 5,50 milhões/mm³
HCM                     29,9 pg                         De 27,0 a 32,0 pg
CHCM                    35,1 g/dL                       De 31,5 a 34,5 g/dL
SÉRIE BRANCA
LEUCÓCITOS           5.000 /mm³                                   3.600 a 12.000 /mm³
SEGMENTADOS          45,0 %          2.250 /mm³         40,0 a 75,0 %         1.440 a 9.000 /mm³
SÉRIE  PLAQUETÁRIA                                            Valor de referência
Plaquetas: 203.000 /mm³                                 De 150.000 a 400.000 /mm³
Método..: RESISTIVIDADE - IMPEDÂNCIA - MICROSCOPIA
Material: SANGUE TOTAL COM EDTA
Hash laudo: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDE
HEMOGLOBINA GLICADA - A1C
Material: Sangue total EDTA
Método..: Turbidimetria                                            Valor de referência
HBA1C:                                                4.78 %              Normal: Inferior a 5,7%
                                                                           Risco aumentado para Diabetes Mellitus: 5,7 a 6,4%
Glicemia estimada média:                       90 mg/dL
Na ausência de hiperglicemia inequívoca, o diagnóstico de diabetes requer dois testes
Endocrinol Metab. 2018 62(4): 386-91
COLESTEROL HDL
Material..: Soro
Método....: Colorimétrico Enzimático                               Valor de referência
Resultado:      53 mg/dL                                                        Sem risco    Risco moderado   Risco elevado
                                                                           Mulheres > 65 mg/dL    45-65 mg/dL       < 45 mg/dL
                                                                           Homens   > 55 mg/dL    35-55 mg/dL       < 35 mg/dL
CREATININA
Material..: Soro
Resultado:        1.24 mg/dL                                                   De 0.70 a 1.20       mg/dL
TSH
Material..: Soro
Resultado:    2.49 µUI/mL                                              0 a 3 dias.................: 1.10 a 15.70 µUI/mL
                                                                           15 a 60 anos...............: 0.40 a 4.30 µUI/mL
                                                                           60 a 80 anos...............: 0.40 a 5.80 µUI/mL
                                                                           Gestantes:
                                                                           Primeiro Trimestre.........: 0.10 a 3.60 µUI/mL
VITAMINA 25 D
Material..: Soro
Resultado:    64,5 ng/mL                                               Desejável para população normal: Acima 20.0 ng/mL
                                                                        Desejável para grupos de risco.: De 30.0 a 60.0 ng/mL
";

    const PAGE2: &str = "\
Paciente                                                                    Sexo  Data de Emissão
JOAO DA SILVA TESTE                                                 M  14/12/2025 08:55:16
Data de Nascimento
25/03/1992                                                    0000002          10000001
UNIMED NOROESTE CAPIXABA COOPERATIVA DE TRABALHO     COLATINA
Exame:                                           Resultado               Valor(es) de referência:
TESTOSTERONA LIVRE
 Data de coleta:   12/12/2025 08:24:11
 Data de recebimento:    13/12/2025 04:35:30
 Testosterona Total                                   337,0 ng/dL                  Testosterona Total
                                                                                   Feminino:
 Globulina Ligadora de Hormônios                      29,0 nmol/L                  20 a 50 anos: 8,4 a 48,1 ng/dL
 Sexuais (SHBG)                                                                   Masculino:
 Testosterona Livre                                    7,23 ng/dL                   20 a 50 anos: 249 a 836 ng/dL
                                                                                   SHBG
                                                                                   Mulheres acima de 18 anos: 18 - 144 nmol/L
                                                                                   Homens: 10 - 57 nmol/L
                                                                                   Testosterona Livre
                                                                                   Feminino:
                                                                                   Fase folicular: 0,18 a 1,68 ng/dL
                                                                                   Masculino:
                                                                                   17 a 40 anos: 3,4 a 24,6 ng/dL
                                                                                   41 a 60 anos: 2,67 a 18,3 ng/dL
Observação:
Comentários:
I. O cálculo de testosterona livre fornece uma estimativa confiável da fração livre
Método: Calculado
                                                                                                        0123456789abcdef0123456789abcdef
Paciente                                                                    Sexo  Data de Emissão
JOAO DA SILVA TESTE                                                 M  18/12/2025 05:19:45
Exame:                                           Resultado               Valor(es) de referência:
ZINCO SÉRICO
 Data de coleta:     12/12/2025 08:24:11
 ZINCO SÉRICO                                      75,10 ug/dL                  70,0 a 120,0 ug/dL (Carência:abaixo de 30,0 ug/dL)
                                                                                Até 170,0 ug/dL (Exposição ocupacional)
Observação:
Ensaio desenvolvido e validado pelo laboratório.
";

    fn parse() -> ParsedReport {
        parse_pages(&[PAGE1.into(), PAGE2.into()], &Patient::default()).unwrap()
    }

    fn find<'a>(r: &'a ParsedReport, key: &str) -> &'a LabResult {
        r.results
            .iter()
            .find(|x| x.biomarker == key)
            .unwrap_or_else(|| panic!("{key} not found in {:#?}", r.results))
    }

    fn range(min: Option<f64>, max: Option<f64>) -> Range {
        Range { min, max }
    }

    #[test]
    fn parses_header() {
        let r = parse();
        assert_eq!(r.lab, "Unimed");
        assert_eq!(r.patient_name.as_deref(), Some("JOAO DA SILVA TESTE"));
        assert_eq!(r.order_no.as_deref(), Some("10000001"));
        assert_eq!(r.birth_date, NaiveDate::from_ymd_opt(1992, 3, 25));
        assert_eq!(r.sex, Some(Sex::Male));
        assert_eq!(r.collected_on, NaiveDate::from_ymd_opt(2025, 12, 12));
    }

    #[test]
    fn parses_blood_count() {
        let r = parse();
        assert_eq!(find(&r, "rbc").value, Some(5.49));
        assert_eq!(find(&r, "rbc").reference, range(Some(4.5), Some(5.5)));
        assert_eq!(find(&r, "mch").reference, range(Some(27.0), Some(32.0)));
        let mchc = find(&r, "mchc");
        assert_eq!((mchc.value, mchc.unit.as_str()), (Some(35.1), "%"));
        assert_eq!(find(&r, "wbc").value, Some(5000.0));
        assert_eq!(find(&r, "neutrophils_abs").value, Some(2250.0));
        assert_eq!(
            find(&r, "neutrophils_pct").reference,
            range(Some(40.0), Some(75.0))
        );
        let plt = find(&r, "platelets");
        assert_eq!(plt.value, Some(203_000.0));
        assert_eq!(plt.reference, range(Some(150_000.0), Some(400_000.0)));
    }

    #[test]
    fn parses_named_and_resultado_lines() {
        let r = parse();
        let a1c = find(&r, "hba1c");
        assert_eq!(a1c.value, Some(4.78));
        assert_eq!(a1c.reference, range(None, Some(5.7)));
        assert!(a1c.reference_text.contains("Risco aumentado"));
        assert_eq!(find(&r, "eag").value, Some(90.0));

        let hdl = find(&r, "hdl");
        assert_eq!(hdl.value, Some(53.0));
        assert_eq!(hdl.reference, range(Some(55.0), None), "male, 'sem risco'");

        let crea = find(&r, "creatinine");
        assert_eq!(crea.value, Some(1.24));
        assert_eq!(crea.reference, range(Some(0.7), Some(1.2)));

        let tsh = find(&r, "tsh");
        assert_eq!(tsh.value, Some(2.49));
        assert_eq!(tsh.reference, range(Some(0.4), Some(4.3)), "30 years old");

        let vitd = find(&r, "vitamin_d");
        assert_eq!(vitd.value, Some(64.5));
        assert_eq!(vitd.reference, range(Some(20.0), None));
    }

    #[test]
    fn parses_partner_lab_pages() {
        let r = parse();
        let total = find(&r, "testosterone");
        assert_eq!(total.value, Some(337.0));
        assert_eq!(total.reference, range(Some(249.0), Some(836.0)));

        let shbg = find(&r, "shbg");
        assert_eq!(
            shbg.original_name,
            "Globulina Ligadora de Hormônios Sexuais (SHBG)"
        );
        assert_eq!(shbg.value, Some(29.0));
        assert_eq!(shbg.reference, range(Some(10.0), Some(57.0)));

        let free = find(&r, "free_testosterone");
        assert_eq!(free.value, Some(7.23));
        assert_eq!(free.reference, range(Some(3.4), Some(24.6)));

        let zinc = find(&r, "zinc");
        assert_eq!(zinc.value, Some(75.1));
        assert_eq!(zinc.reference, range(Some(70.0), Some(120.0)));
        assert_eq!(zinc.exam, "ZINCO SÉRICO");
    }

    #[test]
    fn ignores_notes_and_headers() {
        let r = parse();
        assert!(
            !r.results
                .iter()
                .any(|x| x.original_name.contains("Endocrinol")
                    || x.original_name.contains("ROSIANE")
                    || x.original_name.contains("JOAO")),
            "{:#?}",
            r.results
        );
        assert_eq!(r.results.len(), 17, "{:#?}", r.results);
    }
}
