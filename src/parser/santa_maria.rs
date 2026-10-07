//! Laboratório Santa Maria (Colatina/ES, lacsm.com.br) result reports.
//!
//! Two result layouts appear in these reports:
//! - table rows: `HEMOGLOBINA........:  15,8  g/dL   13,9 a 16,3 g/dL`
//!   (blood counts may carry `%` and absolute values on the same row);
//! - single tests: an exam title, `RESULTADO: 77 mg/dL` (or sub-results such
//!   as `BILIRRUBINA 1,80 mg/dL` + `TOTAL:`), then `Valor de referência:` with
//!   one or more (indented) lines.

#[cfg(test)]
use chrono::NaiveDate;

use super::common::{collapse_spaces, first_date, is_upper_words, parse_row};
use super::{LabParser, ParsedReport};
use crate::model::{LabResult, Sex};
use crate::units::{Range, parse_br_number};

pub struct SantaMaria;

impl LabParser for SantaMaria {
    fn lab_name(&self) -> &'static str {
        "Laboratório Santa Maria"
    }

    fn detect(&self, text: &str) -> bool {
        text.contains("lacsm.com.br") || text.contains("LABORATÓRIO SANTA MARIA")
    }

    fn parse(&self, pages: &[String]) -> ParsedReport {
        let mut p = Parser::default();
        for page in pages {
            for line in page.lines() {
                p.line(line);
            }
        }
        p.flush_reference();
        p.report
    }
}

/// Lines that are page headers/footers or notes and never carry results.
const BOILERPLATE: &[&str] = &[
    "Médico",
    "Convênio",
    "Pedido",
    "Método",
    "Assinatura",
    "ASSINATURA",
    "LAUDO ASSINADO",
    "Responsáveis",
    "Os valores",
    "etc.",
    "CNES",
    "UNIDADE MATRIZ",
    "TELEFONES",
    "acesse nosso",
    "Nota",
    "Obs",
    "Exame realizado",
    "Impresso",
];

/// Indentation (in columns) above which a line continues a reference block.
const CONTINUATION_INDENT: usize = 8;

#[derive(Default)]
struct Parser {
    report: ParsedReport,
    exam: String,
    /// Index (in `report.results`) of the first result of the current exam.
    exam_start: usize,
    /// Index of the `RESULTADO:` result of the current exam, if any.
    main_result: Option<usize>,
    /// Last meaningful line, candidate exam title.
    prev_line: String,
    /// Open `Valor de referência` block.
    reference: Option<Vec<String>>,
    /// A sub-result whose name may continue on the next line (`TOTAL:`).
    pending_name: Option<usize>,
}

impl Parser {
    fn line(&mut self, raw: &str) {
        let t = raw.trim();
        if t.is_empty() {
            return;
        }
        let indent = raw.len() - raw.trim_start().len();

        // `BILIRRUBINA 1,80 mg/dL` followed by `TOTAL:`.
        if let Some(i) = self.pending_name.take()
            && let Some(suffix) = t.strip_suffix(':')
            && is_upper_words(suffix)
        {
            let r = &mut self.report.results[i];
            r.original_name = format!("{} {}", r.original_name, suffix.trim());
            return;
        }

        if t.starts_with("Nome") && t.contains(':') {
            self.flush_reference();
            self.header_name(t);
            return;
        }
        if t.contains("DN/Sexo") {
            self.header_birth(t);
        }
        if t.starts_with("Data OS") {
            self.flush_reference();
            if self.report.collected_on.is_none() {
                self.report.collected_on = first_date(t);
            }
            return;
        }
        if t.starts_with("Material") {
            self.flush_reference();
            self.start_exam(t);
            return;
        }
        if t.starts_with('_') || BOILERPLATE.iter().any(|b| t.starts_with(b)) {
            self.flush_reference();
            return;
        }

        if let Some(rest) = strip_reference_label(t) {
            self.flush_reference();
            let mut lines = Vec::new();
            if !rest.trim().is_empty() {
                lines.push(rest.trim().to_string());
            }
            self.reference = Some(lines);
            return;
        }

        if self.table_row(t) {
            self.flush_reference();
            return;
        }

        if let Some(rest) = t.strip_prefix("RESULTADO") {
            self.flush_reference();
            let rest = rest.trim_start_matches(['.', ':', ' ']);
            let (value, text_value, unit) = split_value(rest);
            self.main_result = Some(self.report.results.len());
            self.push(
                self.exam.clone(),
                value,
                text_value,
                unit,
                Range::default(),
                "",
            );
            return;
        }

        if let Some((name, value, unit)) = sub_result(t) {
            self.flush_reference();
            self.pending_name = Some(self.report.results.len());
            self.push(name, Some(value), None, unit, Range::default(), "");
            return;
        }

        if let Some(lines) = &mut self.reference {
            if indent >= CONTINUATION_INDENT {
                lines.push(t.to_string());
                return;
            }
            self.flush_reference();
        }
        self.prev_line = t.to_string();
    }

    fn header_name(&mut self, t: &str) {
        let after = t.split_once(':').map_or("", |(_, r)| r);
        let (name, order) = match after.split_once("OS:") {
            Some((n, o)) => (n, Some(o.trim().to_string())),
            None => (after, None),
        };
        if self.report.patient_name.is_none() && !name.trim().is_empty() {
            self.report.patient_name = Some(collapse_spaces(name));
        }
        if self.report.order_no.is_none() {
            self.report.order_no = order.filter(|o| !o.is_empty());
        }
    }

    fn header_birth(&mut self, t: &str) {
        let after = t.split("DN/Sexo").nth(1).unwrap_or("");
        if self.report.birth_date.is_none() {
            self.report.birth_date = first_date(after);
        }
        if self.report.sex.is_none() {
            self.report.sex = after.split_whitespace().last().and_then(Sex::from_code);
        }
    }

    fn start_exam(&mut self, t: &str) {
        self.exam = collapse_spaces(&self.prev_line);
        self.exam_start = self.report.results.len();
        self.main_result = None;
        self.pending_name = None;
        if let Some(coleta) = t.split("Coleta").nth(1)
            && let Some(date) = first_date(coleta)
        {
            // The report date is the earliest collection date.
            if self.report.collected_on.is_none_or(|d| date < d) {
                self.report.collected_on = Some(date);
            }
        }
    }

    fn push(
        &mut self,
        name: String,
        value: Option<f64>,
        text_value: Option<String>,
        unit: String,
        reference: Range,
        reference_text: &str,
    ) {
        if name.is_empty() || (value.is_none() && text_value.is_none()) {
            return;
        }
        self.report.results.push(LabResult {
            biomarker: String::new(),
            original_name: name,
            exam: self.exam.clone(),
            value,
            text_value,
            unit,
            reference,
            reference_text: reference_text.to_string(),
        });
    }

    /// Parses `NAME.....: value unit [value unit] min a max unit [...]`.
    fn table_row(&mut self, t: &str) -> bool {
        let Some(dots) = t.find("..") else {
            return false;
        };
        let name = t[..dots].trim();
        let rest = t[dots..].trim_start_matches('.');
        let Some(rest) = rest.strip_prefix(':') else {
            return false;
        };
        if name.is_empty() || !is_upper_words(name) {
            return false;
        }
        let Some(row) = parse_row(rest) else {
            return false;
        };
        let name = collapse_spaces(name);
        for (idx, (value, unit)) in row.values.into_iter().enumerate() {
            let (range, text) = row.ranges.get(idx).cloned().unwrap_or_default();
            self.push(name.clone(), Some(value), None, unit, range, &text);
        }
        true
    }

    /// Assigns the collected reference block to the right result: the exam's
    /// `RESULTADO` if it has no reference yet, otherwise the last result
    /// without one.
    fn flush_reference(&mut self) {
        let Some(lines) = self.reference.take() else {
            return;
        };
        if lines.is_empty() {
            return;
        }
        let results = &mut self.report.results;
        let target = self
            .main_result
            .filter(|&i| results[i].reference_text.is_empty())
            .or_else(|| {
                (self.exam_start..results.len())
                    .rev()
                    .find(|&i| results[i].reference_text.is_empty())
            });
        // The range itself is resolved later, knowing the patient's sex/age.
        if let Some(i) = target {
            results[i].reference_text = lines.join("\n");
        }
    }
}

fn strip_reference_label(t: &str) -> Option<&str> {
    let rest = t
        .strip_prefix("Valor de referência")
        .or_else(|| t.strip_prefix("Valores de referência"))
        .or_else(|| t.strip_prefix("VALOR DE REFERÊNCIA"))?;
    Some(rest.trim_start_matches(['.', ':', ' ']))
}

/// `77 mg/dL` → value + unit; `NÃO REAGENTE` → text value.
fn split_value(rest: &str) -> (Option<f64>, Option<String>, String) {
    let mut tokens = rest.split_whitespace();
    let Some(first) = tokens.next() else {
        return (None, None, String::new());
    };
    match parse_br_number(first) {
        Some(v) => {
            let unit = tokens.collect::<Vec<_>>().join(" ");
            let unit = if first.ends_with('%') && unit.is_empty() {
                "%".to_string()
            } else {
                unit
            };
            (Some(v), None, unit)
        }
        None => (None, Some(collapse_spaces(rest)), String::new()),
    }
}

/// `BILIRRUBINA    1,80 mg/dL` / `GLICEMIA MÉDIA  73,80  mg/dL`.
fn sub_result(t: &str) -> Option<(String, f64, String)> {
    let tokens: Vec<&str> = t.split_whitespace().collect();
    let num_at = tokens
        .iter()
        .position(|tok| parse_br_number(tok).is_some())?;
    if num_at == 0 || tokens.len() != num_at + 2 {
        return None;
    }
    let name = tokens[..num_at].join(" ");
    let name = name.trim_end_matches([':', '.']);
    if !is_upper_words(name) {
        return None;
    }
    let value = parse_br_number(tokens[num_at])?;
    Some((name.to_string(), value, tokens[num_at + 1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic report in the same layout as the lab's PDFs (fake patient).
    const PAGE1: &str = "\
 Nome....:  MARIA DA SILVA TESTE                                     OS: 012-00000-001
 Médico..:  DR. FULANO                         DN/Sexo......:  15/03/1990  35 anos  F
 Convênio:  PARTICULAR
 Data OS.:  09/09/2025 - 17:18                 Impresso em..:  07/10/2026 - 14:05
 ____________________________________________________________________1/2
 HEMOGRAMA COMPLETO
 Material:  Sangue total com EDTA                          Coleta...:  09/09/2025 - 17:19
 Método..:  Citometria de Fluxo Fluorescente               Liberação:  09/09/2025 - 17:57
 ERITROGRAMA                                               Valor de referência:
 HEMÁCIAS...........:        5,26  milhões/mm³              4,50  a  5,90 milhões/mm³
 HEMOGLOBINA........:        12,1  g/dL                     12,0  a  15,5  g/dL
 LEUCOGRAMA
 Valor de referência:
 LEUCÓCITOS TOTAIS..:                  6.890  mm³          4.500 a 11.000 mm³
 N. SEGMENTADOS.....:   48,4  %        3.330  mm³   34,0 a 64,0 %   1.620 a  7.260 mm³
 MONÓCITOS..........:   12,9  %          890  mm³    4,0 a 10,0 %     180 a  1.100 mm³
 PLAQUETAS..........:                198.000  mm³      150.000 a 450.000 mm³
 Assinatura digital avançada: 3005AAD788
 acesse nosso site: lacsm.com.br
";

    const PAGE2: &str = "\
 Nome....:  MARIA DA SILVA TESTE                                     OS: 012-00000-001
 GLICOSE EM JEJUM
 Material: Soro                                            Coleta...:  09/09/2025 - 17:19
 Método..: Enzimático-colorimétrico                        Liberação:  09/09/2025 - 18:49
 RESULTADO:                              77  mg/dL
 Valor de referência: normal........................: DE  60 a  99 mg/dL
                      glicose de jejum alterada.....: DE 100 a 125 mg/dL
                      sugestivo de diabetes mellitus:     >= a 126 mg/dL
 O diagnostico de Diabetes Melittus deve sempre ser confirmado.
 HEMOGLOBINA GLICADA
 Material: Sangue total com EDTA                           Coleta....: 09/09/2025 - 17:19
 RESULTADO:                  4,20  %
 GLICEMIA MÉDIA             73,80  mg/dL
 ESTIMADA:
 Valor de referência: menor que 5,7%
                      diabetes mellitus: pré-diabetes: 5,7% a 6,4%
 Nota:
 BILIRRUBINA TOTAL E FRAÇÕES
 Material: Soro                                            Coleta....: 09/09/2025 - 17:19
   BILIRRUBINA                                1,80 mg/dL
 TOTAL:
 Valor de referência: Até 1,3 mg/dL
 BILIRRUBINA                                  0,80 mg/dL
 DIRETA:
 Valor de referência: Até 0,4 mg/dL
 TRIGLICERÍDEOS
 Material: Soro                                            Coleta....: 09/09/2025 - 17:19
 RESULTADO:                                          60 mg/dL
 Valor de referência: Desejável.: menor que 150 mg/dL
                      Alto......: 200 a 499 mg/dL
 PROLACTINA
 Material: Soro                                            Coleta....: 09/09/2025 - 17:19
 RESULTADO:                                      9,20 ng/mL
 Valor de referência: HOMEM.: DE 3,4 A 19,4 ng/mL
                      MULHER: DE 4,8 A 23,3 ng/mL
 VITAMINA D 25-HIDROXI
 Material: Soro                                            Coleta....: 09/09/2025 - 17:19
 RESULTADO:                                    40,00        ng/mL
 Valor de referência:
                        DEFICIENTE.....................: INFERIOR A 20,0 ng/mL
                        ADEQUADO PARA A POPULAÇÃO GERAL: ENTRE 20,0 A 60,0 ng/mL
 HIV 1/2 ANTICORPOS
 Material: Soro                                            Coleta....: 09/09/2025 - 17:19
 RESULTADO:                                    NÃO REAGENTE
 acesse nosso site: lacsm.com.br
";

    fn parse() -> ParsedReport {
        super::super::parse_pages(
            &[PAGE1.into(), PAGE2.into()],
            &super::super::Patient::default(),
        )
        .unwrap()
    }

    fn find<'a>(r: &'a ParsedReport, key: &str) -> &'a LabResult {
        r.results
            .iter()
            .find(|x| x.biomarker == key)
            .unwrap_or_else(|| panic!("{key} not found in {:#?}", r.results))
    }

    #[test]
    fn parses_header() {
        let r = parse();
        assert_eq!(r.lab, "Laboratório Santa Maria");
        assert_eq!(r.patient_name.as_deref(), Some("MARIA DA SILVA TESTE"));
        assert_eq!(r.order_no.as_deref(), Some("012-00000-001"));
        assert_eq!(r.birth_date, NaiveDate::from_ymd_opt(1990, 3, 15));
        assert_eq!(r.sex, Some(Sex::Female));
        assert_eq!(r.collected_on, NaiveDate::from_ymd_opt(2025, 9, 9));
    }

    #[test]
    fn parses_table_rows() {
        let r = parse();
        let hb = find(&r, "hemoglobin");
        assert_eq!(hb.value, Some(12.1));
        assert_eq!(hb.unit, "g/dL");
        assert_eq!(hb.reference.min, Some(12.0));
        assert_eq!(hb.reference.max, Some(15.5));
        assert_eq!(hb.exam, "HEMOGRAMA COMPLETO");

        let wbc = find(&r, "wbc");
        assert_eq!(wbc.value, Some(6890.0));
        assert_eq!(wbc.unit, "/mm³");
        assert_eq!(wbc.reference.max, Some(11000.0));

        let seg_pct = find(&r, "neutrophils_pct");
        assert_eq!(seg_pct.value, Some(48.4));
        assert_eq!(seg_pct.reference.max, Some(64.0));
        let seg_abs = find(&r, "neutrophils_abs");
        assert_eq!(seg_abs.value, Some(3330.0));
        assert_eq!(seg_abs.reference.min, Some(1620.0));

        assert_eq!(find(&r, "platelets").value, Some(198_000.0));
        assert_eq!(find(&r, "monocytes_pct").reference.max, Some(10.0));
    }

    #[test]
    fn parses_single_results_and_references() {
        let r = parse();
        let glu = find(&r, "glucose");
        assert_eq!(glu.value, Some(77.0));
        assert_eq!(glu.reference.min, Some(60.0));
        assert_eq!(glu.reference.max, Some(99.0));
        assert!(glu.reference_text.contains("glicose de jejum alterada"));

        let a1c = find(&r, "hba1c");
        assert_eq!(a1c.value, Some(4.2));
        assert_eq!(a1c.unit, "%");
        assert_eq!(a1c.reference.max, Some(5.7));
        let eag = find(&r, "eag");
        assert_eq!(eag.original_name, "GLICEMIA MÉDIA ESTIMADA");
        assert!(eag.reference_text.is_empty());

        let tg = find(&r, "triglycerides");
        assert_eq!(tg.reference.max, Some(150.0));
        assert_eq!(tg.reference.min, None);

        let prl = find(&r, "prolactin");
        assert_eq!(prl.reference.min, Some(4.8), "female range");

        let vitd = find(&r, "vitamin_d");
        assert_eq!(vitd.reference.min, Some(20.0));
        assert_eq!(vitd.reference.max, Some(60.0));
    }

    #[test]
    fn parses_sub_results() {
        let r = parse();
        let total = find(&r, "bilirubin_total");
        assert_eq!(total.value, Some(1.8));
        assert_eq!(total.reference.max, Some(1.3));
        let direct = find(&r, "bilirubin_direct");
        assert_eq!(direct.value, Some(0.8));
        assert_eq!(direct.reference.max, Some(0.4));
    }

    #[test]
    fn keeps_text_and_unknown_results() {
        let r = parse();
        let hiv = r
            .results
            .iter()
            .find(|x| x.original_name == "HIV 1/2 ANTICORPOS")
            .unwrap();
        assert_eq!(hiv.text_value.as_deref(), Some("NÃO REAGENTE"));
        assert!(hiv.biomarker.starts_with("custom:"));
    }

    #[test]
    fn does_not_mistake_reference_lines_for_results() {
        let r = parse();
        assert!(
            !r.results
                .iter()
                .any(|x| x.original_name.to_uppercase().starts_with("ALTO")
                    || x.original_name.contains("normal"))
        );
        assert_eq!(r.results.len(), 17, "{:#?}", r.results);
    }
}
