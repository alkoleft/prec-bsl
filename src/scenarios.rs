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
mod tests {
    use super::*;

    const EXPECTED_REQUIRED_V1: &[&str] = &[
        "ВставкаКопирайтов",
        "ДобавлениеПробеловПередКлючевымиСловами",
        "ЗапретИспользованияПерейти",
        "ИсправлениеНеКаноническогоНаписания",
        "КорректировкаXMLФорм",
        "ОбработкаЮнитТестов",
        "ОтключениеПолнотекстовогоПоиска",
        "ОтключениеРазрешенияИзменятьФорму",
        "ПроверкаДублейПроцедурИФункций",
        "ПроверкаКорректностиИнструкцийПрепроцессора",
        "ПроверкаКорректностиОбластей",
        "ПроверкаНецензурныхСлов",
        "РазборОтчетовОбработокРасширений",
        "СинхронизацияОбъектовМетаданныхИФайлов",
        "СортировкаСостава",
        "УдалениеДублейМетаданных",
        "УдалениеЛишнихКонцевыхПробелов",
        "УдалениеЛишнихПустыхСтрок",
    ];

    const EXPECTED_COMPATIBILITY: &[&str] =
        &["СортировкаДереваМетаданных", "СортировкаСоставаПодсистем"];

    #[test]
    fn scenario_inventory_matches_required_v1_list() {
        let required = REFERENCE_SCENARIOS
            .iter()
            .filter(|scenario| scenario.support == ScenarioSupport::RequiredV1)
            .map(|scenario| scenario.id)
            .collect::<Vec<_>>();

        assert_eq!(required, EXPECTED_REQUIRED_V1);
    }

    #[test]
    fn scenario_inventory_keeps_unsupported_ordinary_forms_explicit() {
        let scenario = find_reference_scenario("РазборОбычныхФормНаИсходники.os").unwrap();

        assert_eq!(scenario.id, UNSUPPORTED_ORDINARY_FORMS);
        assert_eq!(scenario.support, ScenarioSupport::Unsupported);
    }

    #[test]
    fn scenario_inventory_keeps_explicit_compatibility_scenarios_out_of_required_v1() {
        let compatibility = REFERENCE_SCENARIOS
            .iter()
            .filter(|scenario| scenario.support == ScenarioSupport::Compatibility)
            .map(|scenario| scenario.id)
            .collect::<Vec<_>>();

        assert_eq!(compatibility, EXPECTED_COMPATIBILITY);
        for scenario in compatibility {
            assert!(!EXPECTED_REQUIRED_V1.contains(&scenario));
        }
    }

    #[test]
    fn scenario_lookup_accepts_ids_with_and_without_os_suffix() {
        let plain = find_reference_scenario("УдалениеЛишнихКонцевыхПробелов").unwrap();
        let suffixed = find_reference_scenario("УдалениеЛишнихКонцевыхПробелов.os").unwrap();

        assert_eq!(plain, suffixed);
    }

    #[test]
    fn scenario_inventory_fixture_preserves_reference_config_key_and_order() {
        let fixture = include_str!("../tests/fixtures/scenario_inventory/reference-v8config.json");

        assert!(fixture.contains("Precommt4onecСценарии"));
        assert!(fixture.contains("РазборОбычныхФормНаИсходники.os"));

        let mut previous_position = 0;
        for scenario in REFERENCE_SCENARIOS
            .iter()
            .filter(|scenario| scenario.support != ScenarioSupport::Compatibility)
        {
            let position = fixture[previous_position..]
                .find(scenario.source_file)
                .unwrap_or_else(|| {
                    panic!("missing scenario fixture entry: {}", scenario.source_file)
                });
            previous_position += position + scenario.source_file.len();
        }
    }
}
