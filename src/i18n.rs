//! Minimal two-language UI strings (Brazilian Portuguese and English).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Pt,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Pt => "pt",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Lang {
        if code == "en" { Lang::En } else { Lang::Pt }
    }

    pub fn toggle(self) -> Lang {
        match self {
            Lang::Pt => Lang::En,
            Lang::En => Lang::Pt,
        }
    }

    pub fn decimal_comma(self) -> bool {
        self == Lang::Pt
    }

    pub fn date_format(self) -> &'static str {
        match self {
            Lang::Pt => "%d/%m/%Y",
            Lang::En => "%Y-%m-%d",
        }
    }

    pub fn t(self, key: Key) -> &'static str {
        let (pt, en) = text(key);
        match self {
            Lang::Pt => pt,
            Lang::En => en,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Welcome,
    WelcomeHint,
    Name,
    BirthDate,
    Sex,
    Male,
    Female,
    Weight,
    Height,
    OnboardingKeys,
    ProfileTitle,
    ErrName,
    ErrDate,
    ErrDateFuture,
    ErrWeight,
    ErrHeight,
    Years,
    Bmi,
    TabBiomarkers,
    TabInsights,
    TabReports,
    NoData,
    NoDataHint,
    LastValue,
    Change,
    Low,
    High,
    Normal,
    BelowGoal,
    AboveGoal,
    Desirable,
    Unknown,
    Reference,
    Target,
    History,
    Date,
    Value,
    About,
    IfHigh,
    IfLow,
    Disclaimer,
    DashboardKeys,
    ReportsKeys,
    PickFile,
    PickFileKeys,
    Review,
    ReviewKeys,
    Lab,
    Collected,
    Include,
    OriginalName,
    Unit,
    Saved,
    Duplicate,
    ParseFailed,
    NothingFound,
    WeightPrompt,
    TargetPrompt,
    TargetCleared,
    ConfirmDelete,
    Deleted,
    OutOfRange,
    BigChanges,
    LabChanged,
    Derived,
    AllNormal,
    Exported,
    WarnNameMismatch,
    WarnBirthMismatch,
    WarnSexMismatch,
    WarnRepeated,
    BmiUnder,
    BmiNormal,
    BmiOver,
    BmiObese,
    EgfrNote,
    NonHdlNote,
    TgHdlNote,
    HomaNote,
    FriedewaldNote,
    EditValue,
    EditDate,
    Results,
}

fn text(key: Key) -> (&'static str, &'static str) {
    use Key::*;
    match key {
        Welcome => ("Bem-vindo ao B-Health", "Welcome to B-Health"),
        WelcomeHint => (
            "Antes de começar, preencha seus dados básicos. Eles ficam apenas neste computador.",
            "Before starting, fill in your basic information. It stays on this computer only.",
        ),
        Name => ("Nome", "Name"),
        BirthDate => ("Data de nascimento", "Date of birth"),
        Sex => ("Sexo biológico", "Biological sex"),
        Male => ("Masculino", "Male"),
        Female => ("Feminino", "Female"),
        Weight => ("Peso (kg)", "Weight (kg)"),
        Height => ("Altura (cm)", "Height (cm)"),
        OnboardingKeys => (
            "↑/↓ Tab: campo  ←/→ espaço: sexo  Enter: salvar  F2: English  Esc: sair",
            "↑/↓ Tab: field  ←/→ space: sex  Enter: save  F2: Português  Esc: quit",
        ),
        ProfileTitle => ("Perfil", "Profile"),
        ErrName => ("Informe o nome", "Enter a name"),
        ErrDate => ("Data inválida (DD/MM/AAAA)", "Invalid date (DD/MM/YYYY)"),
        ErrDateFuture => (
            "A data não pode estar no futuro",
            "The date cannot be in the future",
        ),
        ErrWeight => ("Peso entre 2 e 400 kg", "Weight between 2 and 400 kg"),
        ErrHeight => ("Altura entre 40 e 250 cm", "Height between 40 and 250 cm"),
        Years => ("anos", "years"),
        Bmi => ("IMC", "BMI"),
        TabBiomarkers => ("Biomarcadores", "Biomarkers"),
        TabInsights => ("Insights", "Insights"),
        TabReports => ("Laudos", "Reports"),
        NoData => ("Nenhum exame importado ainda.", "No tests imported yet."),
        NoDataHint => (
            "Pressione [i] para importar um laudo em PDF.",
            "Press [i] to import a PDF report.",
        ),
        LastValue => ("Último", "Latest"),
        Change => ("Variação", "Change"),
        Low => ("Baixo", "Low"),
        High => ("Alto", "High"),
        Normal => ("Normal", "Normal"),
        BelowGoal => ("Abaixo do desejável", "Below desirable"),
        AboveGoal => ("Acima do desejável", "Above desirable"),
        Desirable => ("desejável", "desirable"),
        Unknown => ("Sem referência", "No reference"),
        Reference => ("Referência do laboratório", "Lab reference"),
        Target => ("Meta pessoal", "Personal target"),
        History => ("Histórico", "History"),
        Date => ("Data", "Date"),
        Value => ("Valor", "Value"),
        About => ("O que é", "What it is"),
        IfHigh => ("Se alto", "If high"),
        IfLow => ("Se baixo", "If low"),
        Disclaimer => (
            "Informativo, não é diagnóstico. Converse com seu médico sobre seus resultados.",
            "For information only, not a diagnosis. Discuss your results with your doctor.",
        ),
        DashboardKeys => (
            "↑↓ navegar  Tab aba  i importar  w peso  t meta  p perfil  L idioma  e exportar  q sair",
            "↑↓ move  Tab tab  i import  w weight  t target  p profile  L language  e export  q quit",
        ),
        ReportsKeys => (
            "↑↓ navegar  Tab aba  d excluir laudo  i importar  q sair",
            "↑↓ move  Tab tab  d delete report  i import  q quit",
        ),
        PickFile => ("Escolha o laudo (PDF)", "Choose the report (PDF)"),
        PickFileKeys => (
            "↑↓ navegar  Enter abrir  Backspace voltar  . ocultos  Esc cancelar",
            "↑↓ move  Enter open  Backspace up  . hidden  Esc cancel",
        ),
        Review => ("Revisar resultados", "Review results"),
        ReviewKeys => (
            "↑↓ navegar  espaço incluir/excluir  e editar valor  d editar data  Enter salvar  Esc cancelar",
            "↑↓ move  space include/exclude  e edit value  d edit date  Enter save  Esc cancel",
        ),
        Lab => ("Laboratório", "Lab"),
        Collected => ("Coleta", "Collected"),
        Include => ("Incl.", "Incl."),
        OriginalName => ("Nome no laudo", "Name on report"),
        Unit => ("Unidade", "Unit"),
        Saved => ("Laudo salvo", "Report saved"),
        Duplicate => ("Este PDF já foi importado", "This PDF was already imported"),
        ParseFailed => ("Não foi possível ler o laudo", "Could not read the report"),
        NothingFound => (
            "Nenhum resultado reconhecido neste PDF (laboratório não suportado?)",
            "No results recognized in this PDF (unsupported lab?)",
        ),
        WeightPrompt => ("Peso de hoje (kg)", "Today's weight (kg)"),
        TargetPrompt => (
            "Meta (mín-máx, ex.: 30-60; vazio para remover)",
            "Target (min-max, e.g. 30-60; empty to remove)",
        ),
        TargetCleared => ("Meta removida", "Target removed"),
        ConfirmDelete => (
            "Excluir este laudo e seus resultados? (s/n)",
            "Delete this report and its results? (y/n)",
        ),
        Deleted => ("Laudo excluído", "Report deleted"),
        OutOfRange => ("Fora da referência", "Out of range"),
        BigChanges => (
            "Mudanças grandes desde o exame anterior",
            "Big changes since the previous test",
        ),
        LabChanged => (
            "laboratórios diferentes: parte da diferença pode vir do método de cada laboratório",
            "different labs: part of the difference may come from each lab's method",
        ),
        Derived => ("Indicadores calculados", "Derived metrics"),
        AllNormal => (
            "Todos os valores do último exame estão dentro da referência.",
            "All values from the latest test are within range.",
        ),
        Exported => ("Exportado para", "Exported to"),
        WarnNameMismatch => (
            "Atenção: o nome no laudo é diferente do perfil",
            "Warning: the name on the report differs from the profile",
        ),
        WarnBirthMismatch => (
            "Atenção: a data de nascimento no laudo é diferente do perfil",
            "Warning: the birth date on the report differs from the profile",
        ),
        WarnRepeated => (
            "Exame repetido no laudo; apenas o primeiro foi marcado",
            "Test repeated in the report; only the first one is checked",
        ),
        WarnSexMismatch => (
            "Atenção: o sexo no laudo é diferente do perfil",
            "Warning: the sex on the report differs from the profile",
        ),
        BmiUnder => ("abaixo do peso", "underweight"),
        BmiNormal => ("peso normal", "normal weight"),
        BmiOver => ("sobrepeso", "overweight"),
        BmiObese => ("obesidade", "obesity"),
        EgfrNote => (
            "Função renal estimada (CKD-EPI 2021). ≥90 normal; 60–89 levemente reduzida; <60 reduzida.",
            "Estimated kidney function (CKD-EPI 2021). ≥90 normal; 60–89 mildly reduced; <60 reduced.",
        ),
        NonHdlNote => (
            "Colesterol total − HDL. Meta usual < 130 mg/dL (risco baixo).",
            "Total cholesterol − HDL. Usual goal < 130 mg/dL (low risk).",
        ),
        TgHdlNote => (
            "Triglicerídeos ÷ HDL. Acima de ~3 sugere resistência à insulina.",
            "Triglycerides ÷ HDL. Above ~3 suggests insulin resistance.",
        ),
        HomaNote => (
            "Glicose × insulina ÷ 405. Acima de ~2,7 sugere resistência à insulina.",
            "Glucose × insulin ÷ 405. Above ~2.7 suggests insulin resistance.",
        ),
        FriedewaldNote => (
            "LDL calculado (Friedewald), pois o laudo não trouxe LDL.",
            "Calculated LDL (Friedewald), as the report had no LDL.",
        ),
        EditValue => ("Novo valor", "New value"),
        EditDate => (
            "Data da coleta (DD/MM/AAAA)",
            "Collection date (DD/MM/YYYY)",
        ),
        Results => ("resultados", "results"),
    }
}
