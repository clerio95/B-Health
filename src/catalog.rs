//! Known biomarkers: canonical keys, bilingual names, panels, aliases used by
//! Brazilian labs and short plain-language notes.

use crate::i18n::Lang;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Panel {
    BloodCount,
    WhiteCells,
    Glucose,
    Lipids,
    Liver,
    Kidney,
    Electrolytes,
    Thyroid,
    Hormones,
    VitaminsIron,
    Inflammation,
    Other,
}

impl Panel {
    pub fn name(self, lang: Lang) -> &'static str {
        use Panel::*;
        match (self, lang) {
            (BloodCount, Lang::Pt) => "Eritrograma e plaquetas",
            (BloodCount, Lang::En) => "Red cells & platelets",
            (WhiteCells, Lang::Pt) => "Leucograma",
            (WhiteCells, Lang::En) => "White cells",
            (Glucose, Lang::Pt) => "Glicemia",
            (Glucose, Lang::En) => "Glucose metabolism",
            (Lipids, Lang::Pt) => "Perfil lipídico",
            (Lipids, Lang::En) => "Lipid panel",
            (Liver, Lang::Pt) => "Fígado",
            (Liver, Lang::En) => "Liver",
            (Kidney, Lang::Pt) => "Rins",
            (Kidney, Lang::En) => "Kidney",
            (Electrolytes, Lang::Pt) => "Eletrólitos e minerais",
            (Electrolytes, Lang::En) => "Electrolytes & minerals",
            (Thyroid, Lang::Pt) => "Tireoide",
            (Thyroid, Lang::En) => "Thyroid",
            (Hormones, Lang::Pt) => "Hormônios",
            (Hormones, Lang::En) => "Hormones",
            (VitaminsIron, Lang::Pt) => "Vitaminas e ferro",
            (VitaminsIron, Lang::En) => "Vitamins & iron",
            (Inflammation, Lang::Pt) => "Inflamação",
            (Inflammation, Lang::En) => "Inflammation",
            (Other, Lang::Pt) => "Outros",
            (Other, Lang::En) => "Other",
        }
    }
}

#[derive(Debug)]
pub struct Biomarker {
    pub key: &'static str,
    pub pt: &'static str,
    pub en: &'static str,
    pub panel: Panel,
    /// Unit values are stored in after normalization (see `units`).
    pub unit: &'static str,
    /// Normalized names (see [`normalize`]) used by labs.
    pub aliases: &'static [&'static str],
    /// Whether this is the percentage variant of a differential count.
    pub percent: bool,
    /// What it measures / what high / what low usually means: (pt, en).
    pub about: (&'static str, &'static str),
    pub high: (&'static str, &'static str),
    pub low: (&'static str, &'static str),
}

impl Biomarker {
    pub fn name(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Pt => self.pt,
            Lang::En => self.en,
        }
    }
}

pub fn pick(lang: Lang, text: (&'static str, &'static str)) -> &'static str {
    match lang {
        Lang::Pt => text.0,
        Lang::En => text.1,
    }
}

macro_rules! bm {
    ($key:literal, $pt:literal, $en:literal, $panel:ident, $unit:literal, [$($alias:literal),* $(,)?],
     $pct:literal, $about:expr, $high:expr, $low:expr) => {
        Biomarker {
            key: $key,
            pt: $pt,
            en: $en,
            panel: Panel::$panel,
            unit: $unit,
            aliases: &[$($alias),*],
            percent: $pct,
            about: $about,
            high: $high,
            low: $low,
        }
    };
}

const NONE: (&str, &str) = ("", "");

const WBC_DIFF_HIGH: (&str, &str) = (
    "Aumentos costumam acompanhar infecções, alergias ou inflamações.",
    "Increases usually accompany infections, allergies or inflammation.",
);
const WBC_DIFF_LOW: (&str, &str) = (
    "Reduções podem ocorrer em infecções virais, uso de medicamentos ou alterações da medula.",
    "Decreases can occur with viral infections, medications or bone-marrow changes.",
);

pub static CATALOG: &[Biomarker] = &[
    // --- Red cells & platelets -------------------------------------------
    bm!(
        "rbc",
        "Hemácias",
        "Red blood cells",
        BloodCount,
        "milhões/mm³",
        ["HEMACIAS", "ERITROCITOS", "GLOBULOS VERMELHOS"],
        false,
        (
            "Quantidade de glóbulos vermelhos, que transportam oxigênio.",
            "Number of red blood cells, which carry oxygen."
        ),
        (
            "Pode indicar desidratação, tabagismo, altitude ou excesso de produção.",
            "May indicate dehydration, smoking, altitude or overproduction."
        ),
        (
            "Pode indicar anemia (falta de ferro, B12, folato, perdas de sangue).",
            "May indicate anemia (iron, B12 or folate deficiency, blood loss)."
        )
    ),
    bm!(
        "hemoglobin",
        "Hemoglobina",
        "Hemoglobin",
        BloodCount,
        "g/dL",
        ["HEMOGLOBINA", "HB", "HGB"],
        false,
        (
            "Proteína das hemácias que carrega o oxigênio. Principal marcador de anemia.",
            "Oxygen-carrying protein in red cells. Main anemia marker."
        ),
        (
            "Pode ocorrer com desidratação, tabagismo ou policitemia.",
            "Can occur with dehydration, smoking or polycythemia."
        ),
        (
            "Sugere anemia; investigar ferro, B12, folato e perdas de sangue.",
            "Suggests anemia; check iron, B12, folate and blood loss."
        )
    ),
    bm!(
        "hematocrit",
        "Hematócrito",
        "Hematocrit",
        BloodCount,
        "%",
        ["HEMATOCRITO", "HT", "HCT"],
        false,
        (
            "Porcentagem do volume do sangue ocupada pelas hemácias.",
            "Share of blood volume made up of red cells."
        ),
        (
            "Pode indicar desidratação ou excesso de hemácias.",
            "May indicate dehydration or excess red cells."
        ),
        (
            "Acompanha a anemia ou excesso de hidratação.",
            "Accompanies anemia or overhydration."
        )
    ),
    bm!(
        "mcv",
        "VCM",
        "MCV",
        BloodCount,
        "fL",
        ["VCM", "VOLUME CORPUSCULAR MEDIO", "MCV"],
        false,
        ("Tamanho médio das hemácias.", "Average size of red cells."),
        (
            "Hemácias grandes: pode sugerir falta de B12/folato, álcool ou alterações da tireoide.",
            "Large cells: may suggest B12/folate deficiency, alcohol or thyroid issues."
        ),
        (
            "Hemácias pequenas: pode sugerir falta de ferro ou talassemia.",
            "Small cells: may suggest iron deficiency or thalassemia."
        )
    ),
    bm!(
        "mch",
        "HCM",
        "MCH",
        BloodCount,
        "pg",
        ["HCM", "HEMOGLOBINA CORPUSCULAR MEDIA", "MCH"],
        false,
        (
            "Quantidade média de hemoglobina em cada hemácia.",
            "Average hemoglobin per red cell."
        ),
        (
            "Costuma acompanhar o VCM alto.",
            "Usually accompanies a high MCV."
        ),
        (
            "Costuma acompanhar falta de ferro.",
            "Usually accompanies iron deficiency."
        )
    ),
    bm!(
        "mchc",
        "CHCM",
        "MCHC",
        BloodCount,
        "%",
        [
            "CHCM",
            "CONCENTRACAO DE HEMOGLOBINA CORPUSCULAR MEDIA",
            "MCHC"
        ],
        false,
        (
            "Concentração de hemoglobina dentro das hemácias.",
            "Hemoglobin concentration inside red cells."
        ),
        (
            "Raro; pode ocorrer em esferocitose ou ser artefato da amostra.",
            "Rare; may occur in spherocytosis or be a sample artifact."
        ),
        (
            "Pode sugerir falta de ferro.",
            "May suggest iron deficiency."
        )
    ),
    bm!(
        "rdw",
        "RDW",
        "RDW",
        BloodCount,
        "%",
        ["RDW"],
        false,
        (
            "Variação de tamanho entre as hemácias.",
            "Variation in red-cell size."
        ),
        (
            "Mistura de hemácias de tamanhos diferentes, comum no início de deficiências nutricionais.",
            "Mixed cell sizes, common early in nutritional deficiencies."
        ),
        NONE
    ),
    bm!(
        "platelets",
        "Plaquetas",
        "Platelets",
        BloodCount,
        "/mm³",
        ["PLAQUETAS", "CONTAGEM DE PLAQUETAS"],
        false,
        (
            "Células que participam da coagulação.",
            "Cells involved in blood clotting."
        ),
        (
            "Pode ocorrer em inflamações, falta de ferro ou após sangramentos.",
            "Can occur with inflammation, iron deficiency or after bleeding."
        ),
        (
            "Maior risco de sangramento quando muito baixas; pode ocorrer em infecções virais (ex.: dengue).",
            "Higher bleeding risk when very low; can occur with viral infections (e.g. dengue)."
        )
    ),
    bm!(
        "mpv",
        "VPM",
        "MPV",
        BloodCount,
        "fL",
        ["MPV", "VPM", "VOLUME PLAQUETARIO MEDIO"],
        false,
        ("Tamanho médio das plaquetas.", "Average platelet size."),
        (
            "Plaquetas jovens/maiores, comum quando há maior produção.",
            "Younger/larger platelets, common with increased production."
        ),
        (
            "Isoladamente costuma ter pouca importância clínica.",
            "On its own it usually has little clinical significance."
        )
    ),
    // --- White cells -------------------------------------------------------
    bm!(
        "wbc",
        "Leucócitos",
        "White blood cells",
        WhiteCells,
        "/mm³",
        ["LEUCOCITOS TOTAIS", "LEUCOCITOS", "GLOBULOS BRANCOS"],
        false,
        (
            "Total de glóbulos brancos, as células de defesa.",
            "Total white blood cells, the immune cells."
        ),
        (
            "Comum em infecções bacterianas, inflamação, estresse ou uso de corticoides.",
            "Common with bacterial infection, inflammation, stress or steroids."
        ),
        (
            "Pode ocorrer em infecções virais, medicamentos ou doenças da medula.",
            "Can occur with viral infections, medications or marrow disease."
        )
    ),
    bm!(
        "band_pct",
        "Bastonetes (%)",
        "Band neutrophils (%)",
        WhiteCells,
        "%",
        ["N BASTONETES", "BASTONETES", "NEUTROFILOS BASTONETES"],
        true,
        (
            "Neutrófilos jovens, liberados em maior número durante infecções.",
            "Young neutrophils, released in larger numbers during infections."
        ),
        (
            "Sugere infecção bacteriana aguda (\"desvio à esquerda\").",
            "Suggests acute bacterial infection (\"left shift\")."
        ),
        NONE
    ),
    bm!(
        "band_abs",
        "Bastonetes",
        "Band neutrophils",
        WhiteCells,
        "/mm³",
        ["N BASTONETES", "BASTONETES", "NEUTROFILOS BASTONETES"],
        false,
        (
            "Neutrófilos jovens, liberados em maior número durante infecções.",
            "Young neutrophils, released in larger numbers during infections."
        ),
        (
            "Sugere infecção bacteriana aguda (\"desvio à esquerda\").",
            "Suggests acute bacterial infection (\"left shift\")."
        ),
        NONE
    ),
    bm!(
        "neutrophils_pct",
        "Neutrófilos segmentados (%)",
        "Segmented neutrophils (%)",
        WhiteCells,
        "%",
        [
            "N SEGMENTADOS",
            "SEGMENTADOS",
            "NEUTROFILOS SEGMENTADOS",
            "NEUTROFILOS"
        ],
        true,
        (
            "Principais células de defesa contra bactérias.",
            "Main defense cells against bacteria."
        ),
        (
            "Comum em infecções bacterianas, inflamação e estresse.",
            "Common with bacterial infection, inflammation and stress."
        ),
        WBC_DIFF_LOW
    ),
    bm!(
        "neutrophils_abs",
        "Neutrófilos segmentados",
        "Segmented neutrophils",
        WhiteCells,
        "/mm³",
        [
            "N SEGMENTADOS",
            "SEGMENTADOS",
            "NEUTROFILOS SEGMENTADOS",
            "NEUTROFILOS"
        ],
        false,
        (
            "Principais células de defesa contra bactérias.",
            "Main defense cells against bacteria."
        ),
        (
            "Comum em infecções bacterianas, inflamação e estresse.",
            "Common with bacterial infection, inflammation and stress."
        ),
        WBC_DIFF_LOW
    ),
    bm!(
        "eosinophils_pct",
        "Eosinófilos (%)",
        "Eosinophils (%)",
        WhiteCells,
        "%",
        ["EOSINOFILOS"],
        true,
        (
            "Células de defesa ligadas a alergias e parasitas.",
            "Immune cells linked to allergies and parasites."
        ),
        (
            "Sugere alergias, asma ou parasitoses.",
            "Suggests allergies, asthma or parasites."
        ),
        NONE
    ),
    bm!(
        "eosinophils_abs",
        "Eosinófilos",
        "Eosinophils",
        WhiteCells,
        "/mm³",
        ["EOSINOFILOS"],
        false,
        (
            "Células de defesa ligadas a alergias e parasitas.",
            "Immune cells linked to allergies and parasites."
        ),
        (
            "Sugere alergias, asma ou parasitoses.",
            "Suggests allergies, asthma or parasites."
        ),
        NONE
    ),
    bm!(
        "basophils_pct",
        "Basófilos (%)",
        "Basophils (%)",
        WhiteCells,
        "%",
        ["BASOFILOS"],
        true,
        (
            "Células de defesa envolvidas em reações alérgicas.",
            "Immune cells involved in allergic reactions."
        ),
        WBC_DIFF_HIGH,
        NONE
    ),
    bm!(
        "basophils_abs",
        "Basófilos",
        "Basophils",
        WhiteCells,
        "/mm³",
        ["BASOFILOS"],
        false,
        (
            "Células de defesa envolvidas em reações alérgicas.",
            "Immune cells involved in allergic reactions."
        ),
        WBC_DIFF_HIGH,
        NONE
    ),
    bm!(
        "monocytes_pct",
        "Monócitos (%)",
        "Monocytes (%)",
        WhiteCells,
        "%",
        ["MONOCITOS"],
        true,
        (
            "Células de defesa que \"limpam\" tecidos e combatem infecções.",
            "Immune cells that clean up tissue and fight infections."
        ),
        (
            "Pode ocorrer na recuperação de infecções, em infecções crônicas ou inflamação.",
            "Can occur when recovering from infections, in chronic infection or inflammation."
        ),
        NONE
    ),
    bm!(
        "monocytes_abs",
        "Monócitos",
        "Monocytes",
        WhiteCells,
        "/mm³",
        ["MONOCITOS"],
        false,
        (
            "Células de defesa que \"limpam\" tecidos e combatem infecções.",
            "Immune cells that clean up tissue and fight infections."
        ),
        (
            "Pode ocorrer na recuperação de infecções, em infecções crônicas ou inflamação.",
            "Can occur when recovering from infections, in chronic infection or inflammation."
        ),
        NONE
    ),
    bm!(
        "lymphocytes_pct",
        "Linfócitos (%)",
        "Lymphocytes (%)",
        WhiteCells,
        "%",
        ["LINFOCITOS", "LINFOCITOS TIPICOS"],
        true,
        (
            "Células de defesa contra vírus e responsáveis pela imunidade.",
            "Immune cells against viruses, responsible for immune memory."
        ),
        (
            "Comum em infecções virais.",
            "Common with viral infections."
        ),
        WBC_DIFF_LOW
    ),
    bm!(
        "lymphocytes_abs",
        "Linfócitos",
        "Lymphocytes",
        WhiteCells,
        "/mm³",
        ["LINFOCITOS", "LINFOCITOS TIPICOS"],
        false,
        (
            "Células de defesa contra vírus e responsáveis pela imunidade.",
            "Immune cells against viruses, responsible for immune memory."
        ),
        (
            "Comum em infecções virais.",
            "Common with viral infections."
        ),
        WBC_DIFF_LOW
    ),
    // --- Glucose -------------------------------------------------------------
    bm!(
        "glucose",
        "Glicose em jejum",
        "Fasting glucose",
        Glucose,
        "mg/dL",
        [
            "GLICOSE EM JEJUM",
            "GLICOSE JEJUM",
            "GLICOSE",
            "GLICEMIA DE JEJUM",
            "GLICEMIA"
        ],
        false,
        ("Açúcar no sangue em jejum.", "Blood sugar while fasting."),
        (
            "Entre 100 e 125 mg/dL sugere pré-diabetes; ≥126 em duas ocasiões sugere diabetes.",
            "100–125 mg/dL suggests prediabetes; ≥126 on two occasions suggests diabetes."
        ),
        (
            "Hipoglicemia: jejum prolongado, medicamentos ou exercício intenso.",
            "Hypoglycemia: prolonged fasting, medication or intense exercise."
        )
    ),
    bm!(
        "hba1c",
        "HbA1c (glicada)",
        "HbA1c",
        Glucose,
        "%",
        [
            "HEMOGLOBINA GLICADA",
            "HEMOGLOBINA GLICOSILADA",
            "HBA1C",
            "A1C"
        ],
        false,
        (
            "Média da glicose dos últimos 2–3 meses.",
            "Average blood sugar over the last 2–3 months."
        ),
        (
            "5,7–6,4% sugere pré-diabetes; ≥6,5% sugere diabetes.",
            "5.7–6.4% suggests prediabetes; ≥6.5% suggests diabetes."
        ),
        (
            "Pode ocorrer em anemias, perdas de sangue ou hemoglobinopatias.",
            "Can occur with anemia, blood loss or hemoglobin disorders."
        )
    ),
    bm!(
        "eag",
        "Glicemia média estimada",
        "Estimated average glucose",
        Glucose,
        "mg/dL",
        [
            "GLICEMIA MEDIA ESTIMADA",
            "GLICEMIA ESTIMADA MEDIA",
            "GLICEMIA MEDIA"
        ],
        false,
        (
            "Glicose média calculada a partir da HbA1c.",
            "Average glucose calculated from HbA1c."
        ),
        NONE,
        NONE
    ),
    bm!(
        "insulin",
        "Insulina",
        "Insulin",
        Glucose,
        "µUI/mL",
        ["INSULINA", "INSULINA BASAL", "INSULINA EM JEJUM"],
        false,
        (
            "Hormônio que leva a glicose para dentro das células.",
            "Hormone that moves glucose into cells."
        ),
        (
            "Em jejum, valores altos sugerem resistência à insulina.",
            "High fasting values suggest insulin resistance."
        ),
        NONE
    ),
    // --- Lipids ----------------------------------------------------------------
    bm!(
        "total_cholesterol",
        "Colesterol total",
        "Total cholesterol",
        Lipids,
        "mg/dL",
        ["COLESTEROL TOTAL"],
        false,
        (
            "Soma de todo o colesterol do sangue.",
            "Sum of all cholesterol in the blood."
        ),
        (
            "Aumenta o risco cardiovascular; avaliar junto com LDL e HDL.",
            "Raises cardiovascular risk; read together with LDL and HDL."
        ),
        NONE
    ),
    bm!(
        "hdl",
        "Colesterol HDL",
        "HDL cholesterol",
        Lipids,
        "mg/dL",
        ["COLESTEROL HDL", "HDL COLESTEROL", "HDL"],
        false,
        (
            "\"Colesterol bom\", que retira colesterol das artérias.",
            "\"Good cholesterol\", removes cholesterol from arteries."
        ),
        NONE,
        (
            "HDL baixo aumenta o risco cardiovascular; exercício ajuda a elevar.",
            "Low HDL raises cardiovascular risk; exercise helps raise it."
        )
    ),
    bm!(
        "ldl",
        "Colesterol LDL",
        "LDL cholesterol",
        Lipids,
        "mg/dL",
        ["COLESTEROL LDL", "LDL COLESTEROL", "LDL"],
        false,
        (
            "\"Colesterol ruim\", que se deposita nas artérias.",
            "\"Bad cholesterol\", deposits in artery walls."
        ),
        (
            "Principal alvo de tratamento para reduzir risco cardiovascular.",
            "Main treatment target to reduce cardiovascular risk."
        ),
        NONE
    ),
    bm!(
        "vldl",
        "Colesterol VLDL",
        "VLDL cholesterol",
        Lipids,
        "mg/dL",
        ["COLESTEROL VLDL", "VLDL COLESTEROL", "VLDL"],
        false,
        (
            "Partículas que transportam principalmente triglicerídeos.",
            "Particles that mostly carry triglycerides."
        ),
        (
            "Acompanha triglicerídeos altos.",
            "Accompanies high triglycerides."
        ),
        NONE
    ),
    bm!(
        "triglycerides",
        "Triglicerídeos",
        "Triglycerides",
        Lipids,
        "mg/dL",
        ["TRIGLICERIDEOS", "TRIGLICERIDES", "TRIGLICERIDIOS"],
        false,
        (
            "Gorduras circulantes no sangue.",
            "Fats circulating in the blood."
        ),
        (
            "Ligado a açúcar/álcool em excesso, sedentarismo e resistência à insulina.",
            "Linked to excess sugar/alcohol, inactivity and insulin resistance."
        ),
        NONE
    ),
    // --- Liver -------------------------------------------------------------------
    bm!(
        "ast",
        "TGO (AST)",
        "AST",
        Liver,
        "U/L",
        [
            "TGO",
            "AST",
            "ASPARTATO AMINOTRANSFERASE",
            "TRANSAMINASE OXALACETICA"
        ],
        false,
        (
            "Enzima do fígado e dos músculos.",
            "Enzyme found in liver and muscle."
        ),
        (
            "Pode indicar lesão do fígado ou dos músculos (inclusive após exercício intenso).",
            "May indicate liver or muscle injury (including after intense exercise)."
        ),
        NONE
    ),
    bm!(
        "alt",
        "TGP (ALT)",
        "ALT",
        Liver,
        "U/L",
        [
            "TGP",
            "ALT",
            "ALANINA AMINOTRANSFERASE",
            "TRANSAMINASE PIRUVICA"
        ],
        false,
        (
            "Enzima mais específica do fígado.",
            "Enzyme more specific to the liver."
        ),
        (
            "Sugere agressão ao fígado: gordura no fígado, álcool, medicamentos, hepatites.",
            "Suggests liver stress: fatty liver, alcohol, medication, hepatitis."
        ),
        NONE
    ),
    bm!(
        "ggt",
        "Gama GT",
        "GGT",
        Liver,
        "U/L",
        ["GAMA GLUTAMIL TRANSFERASE", "GAMA GT", "GGT"],
        false,
        (
            "Enzima do fígado e das vias biliares.",
            "Liver and bile-duct enzyme."
        ),
        (
            "Sensível a álcool, medicamentos, gordura no fígado e obstrução biliar.",
            "Sensitive to alcohol, medication, fatty liver and bile obstruction."
        ),
        NONE
    ),
    bm!(
        "alp",
        "Fosfatase alcalina",
        "Alkaline phosphatase",
        Liver,
        "U/L",
        ["FOSFATASE ALCALINA"],
        false,
        (
            "Enzima do fígado, vias biliares e ossos.",
            "Enzyme from liver, bile ducts and bone."
        ),
        (
            "Pode indicar problemas biliares ou ósseos; normal estar alta na adolescência.",
            "May indicate bile-duct or bone issues; normally high in adolescence."
        ),
        NONE
    ),
    bm!(
        "bilirubin_total",
        "Bilirrubina total",
        "Total bilirubin",
        Liver,
        "mg/dL",
        ["BILIRRUBINA TOTAL"],
        false,
        (
            "Pigmento da degradação das hemácias, eliminado pelo fígado.",
            "Pigment from red-cell breakdown, cleared by the liver."
        ),
        (
            "Se à custa da indireta e com fígado normal, frequentemente é síndrome de Gilbert (benigna).",
            "If mostly indirect with normal liver tests, often Gilbert's syndrome (benign)."
        ),
        NONE
    ),
    bm!(
        "bilirubin_direct",
        "Bilirrubina direta",
        "Direct bilirubin",
        Liver,
        "mg/dL",
        ["BILIRRUBINA DIRETA", "BILIRRUBINA CONJUGADA"],
        false,
        (
            "Fração já processada pelo fígado.",
            "Fraction already processed by the liver."
        ),
        (
            "Pode indicar dificuldade de eliminação pela bile.",
            "May indicate impaired bile excretion."
        ),
        NONE
    ),
    bm!(
        "bilirubin_indirect",
        "Bilirrubina indireta",
        "Indirect bilirubin",
        Liver,
        "mg/dL",
        ["BILIRRUBINA INDIRETA", "BILIRRUBINA NAO CONJUGADA"],
        false,
        (
            "Fração ainda não processada pelo fígado.",
            "Fraction not yet processed by the liver."
        ),
        (
            "Comum na síndrome de Gilbert (benigna), jejum ou destruição de hemácias.",
            "Common in Gilbert's syndrome (benign), fasting or red-cell breakdown."
        ),
        NONE
    ),
    bm!(
        "albumin",
        "Albumina",
        "Albumin",
        Liver,
        "g/dL",
        ["ALBUMINA"],
        false,
        (
            "Principal proteína do sangue, produzida pelo fígado.",
            "Main blood protein, made by the liver."
        ),
        (
            "Geralmente indica desidratação.",
            "Usually indicates dehydration."
        ),
        (
            "Pode indicar desnutrição, inflamação, doença do fígado ou dos rins.",
            "May indicate malnutrition, inflammation, liver or kidney disease."
        )
    ),
    // --- Kidney ----------------------------------------------------------------
    bm!(
        "creatinine",
        "Creatinina",
        "Creatinine",
        Kidney,
        "mg/dL",
        ["CREATININA"],
        false,
        (
            "Resíduo dos músculos eliminado pelos rins; usado para calcular a TFG.",
            "Muscle waste cleared by the kidneys; used to calculate eGFR."
        ),
        (
            "Pode indicar redução da função renal (ou muita massa muscular/creatina).",
            "May indicate reduced kidney function (or high muscle mass/creatine use)."
        ),
        (
            "Comum com pouca massa muscular.",
            "Common with low muscle mass."
        )
    ),
    bm!(
        "urea",
        "Ureia",
        "Urea",
        Kidney,
        "mg/dL",
        ["UREIA", "UREIA SERICA"],
        false,
        (
            "Resíduo das proteínas eliminado pelos rins.",
            "Protein waste cleared by the kidneys."
        ),
        (
            "Desidratação, dieta rica em proteína ou redução da função renal.",
            "Dehydration, high-protein diet or reduced kidney function."
        ),
        NONE
    ),
    bm!(
        "uric_acid",
        "Ácido úrico",
        "Uric acid",
        Kidney,
        "mg/dL",
        ["ACIDO URICO"],
        false,
        (
            "Resíduo do metabolismo das purinas.",
            "Waste product of purine metabolism."
        ),
        (
            "Risco de gota e pedras nos rins; ligado a álcool, frutose e carnes.",
            "Risk of gout and kidney stones; linked to alcohol, fructose and meat."
        ),
        NONE
    ),
    // --- Electrolytes --------------------------------------------------------
    bm!(
        "sodium",
        "Sódio",
        "Sodium",
        Electrolytes,
        "mEq/L",
        ["SODIO"],
        false,
        (
            "Principal sal do sangue; regula a água do corpo.",
            "Main blood salt; regulates body water."
        ),
        ("Geralmente desidratação.", "Usually dehydration."),
        (
            "Excesso de água, diuréticos ou alterações hormonais.",
            "Excess water, diuretics or hormonal changes."
        )
    ),
    bm!(
        "potassium",
        "Potássio",
        "Potassium",
        Electrolytes,
        "mEq/L",
        ["POTASSIO"],
        false,
        (
            "Mineral essencial para músculos e coração.",
            "Mineral essential for muscles and heart."
        ),
        (
            "Pode afetar o ritmo do coração; às vezes é artefato da coleta (hemólise).",
            "Can affect heart rhythm; sometimes a collection artifact (hemolysis)."
        ),
        (
            "Pode causar cãibras e fraqueza; comum com diuréticos ou diarreia.",
            "Can cause cramps and weakness; common with diuretics or diarrhea."
        )
    ),
    bm!(
        "calcium",
        "Cálcio",
        "Calcium",
        Electrolytes,
        "mg/dL",
        ["CALCIO", "CALCIO TOTAL"],
        false,
        (
            "Mineral dos ossos, músculos e nervos.",
            "Mineral for bones, muscles and nerves."
        ),
        (
            "Pode envolver paratireoide ou excesso de vitamina D.",
            "May involve the parathyroid or excess vitamin D."
        ),
        (
            "Pode envolver falta de vitamina D ou albumina baixa.",
            "May involve low vitamin D or low albumin."
        )
    ),
    bm!(
        "magnesium",
        "Magnésio",
        "Magnesium",
        Electrolytes,
        "mg/dL",
        ["MAGNESIO"],
        false,
        (
            "Mineral para músculos, nervos e metabolismo.",
            "Mineral for muscles, nerves and metabolism."
        ),
        NONE,
        (
            "Pode causar cãibras; ligado a álcool, diuréticos e má absorção.",
            "Can cause cramps; linked to alcohol, diuretics and malabsorption."
        )
    ),
    bm!(
        "phosphorus",
        "Fósforo",
        "Phosphorus",
        Electrolytes,
        "mg/dL",
        ["FOSFORO"],
        false,
        (
            "Mineral dos ossos e do metabolismo energético.",
            "Mineral for bones and energy metabolism."
        ),
        NONE,
        NONE
    ),
    // --- Thyroid ---------------------------------------------------------------
    bm!(
        "tsh",
        "TSH",
        "TSH",
        Thyroid,
        "µUI/mL",
        ["TSH", "TSH ULTRA SENSIVEL", "HORMONIO TIREOESTIMULANTE"],
        false,
        (
            "Hormônio que comanda a tireoide; principal triagem da função tireoidiana.",
            "Hormone that drives the thyroid; main thyroid screening test."
        ),
        (
            "Sugere tireoide lenta (hipotireoidismo).",
            "Suggests an underactive thyroid (hypothyroidism)."
        ),
        (
            "Sugere tireoide acelerada (hipertireoidismo) ou excesso de reposição.",
            "Suggests an overactive thyroid or excess replacement."
        )
    ),
    bm!(
        "ft4",
        "T4 livre",
        "Free T4",
        Thyroid,
        "ng/dL",
        ["T4 LIVRE", "TIROXINA LIVRE"],
        false,
        (
            "Hormônio produzido pela tireoide.",
            "Hormone produced by the thyroid."
        ),
        (
            "Junto com TSH baixo sugere hipertireoidismo.",
            "With low TSH suggests hyperthyroidism."
        ),
        (
            "Junto com TSH alto sugere hipotireoidismo.",
            "With high TSH suggests hypothyroidism."
        )
    ),
    bm!(
        "t3",
        "T3",
        "T3",
        Thyroid,
        "ng/dL",
        ["T3", "T3 TOTAL", "TRIIODOTIRONINA"],
        false,
        ("Hormônio ativo da tireoide.", "Active thyroid hormone."),
        NONE,
        NONE
    ),
    // --- Hormones ----------------------------------------------------------------
    bm!(
        "prolactin",
        "Prolactina",
        "Prolactin",
        Hormones,
        "ng/mL",
        ["PROLACTINA"],
        false,
        (
            "Hormônio da hipófise ligado à lactação.",
            "Pituitary hormone linked to lactation."
        ),
        (
            "Estresse, medicamentos ou alterações da hipófise.",
            "Stress, medication or pituitary changes."
        ),
        NONE
    ),
    bm!(
        "testosterone",
        "Testosterona total",
        "Total testosterone",
        Hormones,
        "ng/dL",
        ["TESTOSTERONA TOTAL", "TESTOSTERONA"],
        false,
        ("Principal hormônio androgênico.", "Main androgen hormone."),
        NONE,
        (
            "Pode causar cansaço e baixa libido; influenciado por sono, peso e idade.",
            "Can cause fatigue and low libido; influenced by sleep, weight and age."
        )
    ),
    bm!(
        "free_testosterone",
        "Testosterona livre",
        "Free testosterone",
        Hormones,
        "ng/dL",
        ["TESTOSTERONA LIVRE"],
        false,
        (
            "Fração da testosterona não ligada a proteínas, a forma ativa.",
            "Testosterone not bound to proteins, the active form."
        ),
        NONE,
        (
            "Pode causar cansaço e baixa libido mesmo com a total normal.",
            "Can cause fatigue and low libido even with normal total testosterone."
        )
    ),
    bm!(
        "bioavailable_testosterone",
        "Testosterona biodisponível",
        "Bioavailable testosterone",
        Hormones,
        "ng/dL",
        ["TESTOSTERONA BIODISPONIVEL"],
        false,
        (
            "Testosterona livre somada à ligada à albumina (disponível para os tecidos).",
            "Free plus albumin-bound testosterone (available to tissues)."
        ),
        NONE,
        NONE
    ),
    bm!(
        "shbg",
        "SHBG",
        "SHBG",
        Hormones,
        "nmol/L",
        ["SHBG", "GLOBULINA LIGADORA DE HORMONIOS SEXUAIS"],
        false,
        (
            "Proteína que transporta hormônios sexuais; regula quanto fica livre.",
            "Protein carrying sex hormones; controls how much stays free."
        ),
        (
            "Reduz a fração livre; comum com hipertireoidismo, doença hepática ou idade.",
            "Lowers the free fraction; common with hyperthyroidism, liver disease or age."
        ),
        (
            "Ligado a resistência à insulina e excesso de peso.",
            "Linked to insulin resistance and excess weight."
        )
    ),
    bm!(
        "psa",
        "PSA total",
        "Total PSA",
        Hormones,
        "ng/mL",
        ["PSA TOTAL", "PSA", "ANTIGENO PROSTATICO ESPECIFICO"],
        false,
        ("Marcador da próstata.", "Prostate marker."),
        (
            "Aumento da próstata, inflamação ou, menos comumente, câncer; avaliar com o médico.",
            "Enlarged prostate, inflammation or, less commonly, cancer; discuss with a doctor."
        ),
        NONE
    ),
    // --- Vitamins & iron ---------------------------------------------------
    bm!(
        "vitamin_d",
        "Vitamina D (25-OH)",
        "Vitamin D (25-OH)",
        VitaminsIron,
        "ng/mL",
        [
            "VITAMINA D 25 HIDROXI",
            "25 HIDROXIVITAMINA D",
            "VITAMINA 25 D",
            "VITAMINA D"
        ],
        false,
        (
            "Reserva de vitamina D, importante para ossos e imunidade.",
            "Vitamin D stores, important for bones and immunity."
        ),
        (
            "Acima de 100 ng/mL há risco de intoxicação, geralmente por suplementação.",
            "Above 100 ng/mL there is a toxicity risk, usually from supplements."
        ),
        (
            "Deficiência: pouca exposição ao sol; pode precisar de suplementação.",
            "Deficiency: little sun exposure; may need supplementation."
        )
    ),
    bm!(
        "vitamin_b12",
        "Vitamina B12",
        "Vitamin B12",
        VitaminsIron,
        "pg/mL",
        ["VITAMINA B12", "CIANOCOBALAMINA"],
        false,
        (
            "Vitamina essencial para nervos e formação do sangue.",
            "Vitamin essential for nerves and blood formation."
        ),
        (
            "Geralmente por suplementação.",
            "Usually from supplementation."
        ),
        (
            "Comum em vegetarianos, idosos e uso de metformina ou antiácidos.",
            "Common in vegetarians, older adults and with metformin or antacid use."
        )
    ),
    bm!(
        "folate",
        "Ácido fólico",
        "Folate",
        VitaminsIron,
        "ng/mL",
        ["ACIDO FOLICO", "FOLATO"],
        false,
        (
            "Vitamina B9, essencial para formação das células do sangue.",
            "Vitamin B9, essential for blood-cell formation."
        ),
        NONE,
        (
            "Pode causar anemia; ligado a baixa ingestão de vegetais e álcool.",
            "Can cause anemia; linked to low vegetable intake and alcohol."
        )
    ),
    bm!(
        "ferritin",
        "Ferritina",
        "Ferritin",
        VitaminsIron,
        "ng/mL",
        ["FERRITINA"],
        false,
        ("Reserva de ferro do corpo.", "The body's iron stores."),
        (
            "Inflamação, gordura no fígado ou excesso de ferro.",
            "Inflammation, fatty liver or iron overload."
        ),
        (
            "Estoques de ferro baixos, mesmo antes da anemia aparecer.",
            "Low iron stores, even before anemia appears."
        )
    ),
    bm!(
        "iron",
        "Ferro sérico",
        "Serum iron",
        VitaminsIron,
        "µg/dL",
        ["FERRO SERICO", "FERRO"],
        false,
        (
            "Ferro circulante no sangue (varia ao longo do dia).",
            "Circulating iron (varies through the day)."
        ),
        NONE,
        (
            "Pode indicar deficiência de ferro; avaliar com a ferritina.",
            "May indicate iron deficiency; read together with ferritin."
        )
    ),
    bm!(
        "vitamin_c",
        "Vitamina C",
        "Vitamin C",
        VitaminsIron,
        "mg/dL",
        ["VITAMINA C", "ACIDO ASCORBICO"],
        false,
        (
            "Antioxidante obtido de frutas e vegetais.",
            "Antioxidant from fruit and vegetables."
        ),
        NONE,
        (
            "Baixa ingestão de frutas/vegetais ou tabagismo.",
            "Low fruit/vegetable intake or smoking."
        )
    ),
    bm!(
        "zinc",
        "Zinco",
        "Zinc",
        VitaminsIron,
        "µg/dL",
        ["ZINCO SERICO", "ZINCO"],
        false,
        (
            "Mineral para imunidade, cicatrização e hormônios.",
            "Mineral for immunity, healing and hormones."
        ),
        (
            "Geralmente por suplementação ou exposição ocupacional.",
            "Usually from supplements or occupational exposure."
        ),
        (
            "Pode afetar imunidade, paladar e cicatrização.",
            "Can affect immunity, taste and healing."
        )
    ),
    // --- Inflammation --------------------------------------------------------
    bm!(
        "crp",
        "Proteína C reativa",
        "C-reactive protein",
        Inflammation,
        "mg/L",
        [
            "PROTEINA C REATIVA QUANTITATIVA",
            "PROTEINA C REATIVA ULTRASSENSIVEL",
            "PROTEINA C REATIVA",
            "PCR"
        ],
        false,
        (
            "Marcador geral de inflamação.",
            "General inflammation marker."
        ),
        (
            "Infecções, inflamação ou lesões; valores levemente altos persistentes elevam o risco cardiovascular.",
            "Infection, inflammation or injury; persistently mild elevation raises cardiovascular risk."
        ),
        NONE
    ),
    bm!(
        "esr",
        "VHS",
        "ESR",
        Inflammation,
        "mm/h",
        ["VHS", "VELOCIDADE DE HEMOSSEDIMENTACAO"],
        false,
        (
            "Velocidade de sedimentação, marcador lento de inflamação.",
            "Sedimentation rate, a slow inflammation marker."
        ),
        (
            "Inflamação ou infecção; aumenta com a idade.",
            "Inflammation or infection; rises with age."
        ),
        NONE
    ),
];

/// Uppercases, strips accents and punctuation, and collapses whitespace.
pub fn normalize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'ç' | 'Ç' => 'C',
            c if c.is_alphanumeric() => c.to_ascii_uppercase(),
            _ => ' ',
        };
        if c == ' ' && (out.is_empty() || out.ends_with(' ')) {
            continue;
        }
        out.push(c);
    }
    out.trim_end().to_string()
}

pub fn get(key: &str) -> Option<&'static Biomarker> {
    CATALOG.iter().find(|b| b.key == key)
}

/// Finds the catalog entry for a lab's biomarker name. `unit` disambiguates
/// percentage vs absolute variants of differential counts.
pub fn lookup(name: &str, unit: Option<&str>) -> Option<&'static Biomarker> {
    let norm = normalize(name);
    let is_pct = unit.map(str::trim) == Some("%");
    let matches = |alias: &str| {
        norm == alias
            || norm
                .strip_prefix(alias)
                .is_some_and(|rest| rest.starts_with(' '))
    };
    // Longest alias wins, so "HEMOGLOBINA GLICADA" beats "HEMOGLOBINA".
    let mut best: Option<(&Biomarker, usize)> = None;
    for b in CATALOG {
        for alias in b.aliases {
            if !matches(alias) {
                continue;
            }
            let has_variants = CATALOG
                .iter()
                .any(|o| o.key != b.key && o.aliases.contains(alias));
            if has_variants && b.percent != is_pct {
                continue;
            }
            if best.is_none_or(|(_, len)| alias.len() > len) {
                best = Some((b, alias.len()));
            }
        }
    }
    best.map(|(b, _)| b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize("N. SEGMENTADOS....."), "N SEGMENTADOS");
        assert_eq!(normalize("Ácido Fólico"), "ACIDO FOLICO");
        assert_eq!(
            normalize("TGO-TRANSAMINASE OXALACETICA"),
            "TGO TRANSAMINASE OXALACETICA"
        );
    }

    #[test]
    fn looks_up_by_alias_and_unit() {
        assert_eq!(lookup("HEMOGLOBINA", None).unwrap().key, "hemoglobin");
        assert_eq!(lookup("HEMOGLOBINA GLICADA", None).unwrap().key, "hba1c");
        assert_eq!(
            lookup("N. SEGMENTADOS", Some("%")).unwrap().key,
            "neutrophils_pct"
        );
        assert_eq!(
            lookup("N. SEGMENTADOS", Some("mm³")).unwrap().key,
            "neutrophils_abs"
        );
        assert_eq!(
            lookup("TGO-TRANSAMINASE OXALACETICA", None).unwrap().key,
            "ast"
        );
        assert_eq!(
            lookup("VITAMINA D 25-HIDROXI", None).unwrap().key,
            "vitamin_d"
        );
        assert!(lookup("EXAME DESCONHECIDO", None).is_none());
    }

    #[test]
    fn catalog_keys_are_unique() {
        let mut keys: Vec<_> = CATALOG.iter().map(|b| b.key).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), CATALOG.len());
    }
}
