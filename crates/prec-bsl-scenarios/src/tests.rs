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
    let fixture =
        include_str!("../../../tests/fixtures/scenario_inventory/reference-v8config.json");

    assert!(fixture.contains("Precommt4onecСценарии"));
    assert!(fixture.contains("РазборОбычныхФормНаИсходники.os"));

    let mut previous_position = 0;
    for scenario in REFERENCE_SCENARIOS
        .iter()
        .filter(|scenario| scenario.support != ScenarioSupport::Compatibility)
    {
        let position = fixture[previous_position..]
            .find(scenario.source_file)
            .unwrap_or_else(|| panic!("missing scenario fixture entry: {}", scenario.source_file));
        previous_position += position + scenario.source_file.len();
    }
}
