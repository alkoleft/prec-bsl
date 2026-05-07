#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioSupport {
    RequiredV1,
    Compatibility,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioDefinition {
    pub id: &'static str,
    pub source_file: &'static str,
    pub support: ScenarioSupport,
}

pub const UNSUPPORTED_ORDINARY_FORMS: &str = "РазборОбычныхФормНаИсходники";

pub const REFERENCE_SCENARIOS: &[ScenarioDefinition] = &[
    ScenarioDefinition {
        id: "ВставкаКопирайтов",
        source_file: "ВставкаКопирайтов.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ДобавлениеПробеловПередКлючевымиСловами",
        source_file: "ДобавлениеПробеловПередКлючевымиСловами.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ЗапретИспользованияПерейти",
        source_file: "ЗапретИспользованияПерейти.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ИсправлениеНеКаноническогоНаписания",
        source_file: "ИсправлениеНеКаноническогоНаписания.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "КорректировкаXMLФорм",
        source_file: "КорректировкаXMLФорм.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ОбработкаЮнитТестов",
        source_file: "ОбработкаЮнитТестов.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ОтключениеПолнотекстовогоПоиска",
        source_file: "ОтключениеПолнотекстовогоПоиска.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ОтключениеРазрешенияИзменятьФорму",
        source_file: "ОтключениеРазрешенияИзменятьФорму.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ПроверкаДублейПроцедурИФункций",
        source_file: "ПроверкаДублейПроцедурИФункций.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ПроверкаКорректностиИнструкцийПрепроцессора",
        source_file: "ПроверкаКорректностиИнструкцийПрепроцессора.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ПроверкаКорректностиОбластей",
        source_file: "ПроверкаКорректностиОбластей.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "ПроверкаНецензурныхСлов",
        source_file: "ПроверкаНецензурныхСлов.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: UNSUPPORTED_ORDINARY_FORMS,
        source_file: "РазборОбычныхФормНаИсходники.os",
        support: ScenarioSupport::Unsupported,
    },
    ScenarioDefinition {
        id: "РазборОтчетовОбработокРасширений",
        source_file: "РазборОтчетовОбработокРасширений.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "СинхронизацияОбъектовМетаданныхИФайлов",
        source_file: "СинхронизацияОбъектовМетаданныхИФайлов.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "СортировкаСостава",
        source_file: "СортировкаСостава.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "СортировкаДереваМетаданных",
        source_file: "СортировкаСостава.os",
        support: ScenarioSupport::Compatibility,
    },
    ScenarioDefinition {
        id: "СортировкаСоставаПодсистем",
        source_file: "СортировкаСостава.os",
        support: ScenarioSupport::Compatibility,
    },
    ScenarioDefinition {
        id: "УдалениеДублейМетаданных",
        source_file: "УдалениеДублейМетаданных.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "УдалениеЛишнихКонцевыхПробелов",
        source_file: "УдалениеЛишнихКонцевыхПробелов.os",
        support: ScenarioSupport::RequiredV1,
    },
    ScenarioDefinition {
        id: "УдалениеЛишнихПустыхСтрок",
        source_file: "УдалениеЛишнихПустыхСтрок.os",
        support: ScenarioSupport::RequiredV1,
    },
];

pub fn normalize_scenario_id(value: &str) -> &str {
    value.trim().strip_suffix(".os").unwrap_or(value.trim())
}

pub fn find_reference_scenario(id: &str) -> Option<&'static ScenarioDefinition> {
    let normalized = normalize_scenario_id(id);
    REFERENCE_SCENARIOS
        .iter()
        .find(|scenario| scenario.id == normalized)
}

#[cfg(test)]
mod tests;
