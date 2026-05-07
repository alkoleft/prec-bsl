pub mod app;
pub mod cli;

pub use prec_bsl_bsl::{
    bsl_checkers, bsl_parser, text_checkers, text_fixers, unit_tests_processing,
};
pub use prec_bsl_git as git_index;
pub use prec_bsl_output as output;
pub use prec_bsl_pipeline as scenario_pipeline;
pub use prec_bsl_platform as external_artifacts;
pub use prec_bsl_source as source_files;
pub use prec_bsl_xml::{
    composition_sort, duplicate_metadata, form_change_permission, full_text_search, metadata_sync,
    xml_edt, xml_forms,
};

pub mod config {
    pub use prec_bsl_config::{
        ConfigError, ConfigResolveRequest, ConfigSource, ConfigWarning, GlobalConfig,
        ProjectScenarioConfig, RepositoryScenarioSettings, ResolvedConfig, ScenarioCatalog,
        ScenarioConfig, ScenarioMetadata, ScenarioSupport, UNSUPPORTED_ORDINARY_FORMS,
        normalize_scenario_id,
    };

    pub fn resolve_config(request: &ConfigResolveRequest) -> Result<ResolvedConfig, ConfigError> {
        prec_bsl_config::resolve_config_with_catalog(request, crate::reference_scenario_catalog())
    }

    pub fn parse_config_str(source: &str) -> Result<ResolvedConfig, ConfigError> {
        prec_bsl_config::parse_config_str_with_catalog(source, crate::reference_scenario_catalog())
    }

    pub fn built_in_defaults() -> ResolvedConfig {
        prec_bsl_config::built_in_defaults_with_catalog(crate::reference_scenario_catalog())
    }
}

pub mod scenarios {
    pub use crate::REFERENCE_SCENARIO_METADATA as REFERENCE_SCENARIOS;
    pub use prec_bsl_config::{
        ScenarioMetadata, ScenarioSupport, UNSUPPORTED_ORDINARY_FORMS, normalize_scenario_id,
    };

    pub type ScenarioDefinition = ScenarioMetadata;

    pub fn find_reference_scenario(id: &str) -> Option<&'static ScenarioMetadata> {
        crate::reference_scenario_catalog().find(id)
    }

    pub fn supported_reference_scenarios() -> impl Iterator<Item = ScenarioMetadata> {
        crate::reference_scenario_catalog().supported().copied()
    }
}

pub const UNSUPPORTED_ORDINARY_FORMS_METADATA: config::ScenarioMetadata =
    config::ScenarioMetadata::unsupported(
        config::UNSUPPORTED_ORDINARY_FORMS,
        "РазборОбычныхФормНаИсходники.os",
    );

pub const REFERENCE_SCENARIO_DEFINITIONS: &[scenario_pipeline::ScenarioDefinition] = &[
    text_fixers::COPYRIGHT_SCENARIO,
    text_fixers::KEYWORD_SPACING_SCENARIO,
    bsl_checkers::FORBID_GOTO_SCENARIO,
    text_fixers::CANONICAL_SPELLING_SCENARIO,
    xml_forms::XML_FORM_CORRECTION_SCENARIO,
    unit_tests_processing::UNIT_TESTS_PROCESSING_SCENARIO,
    full_text_search::DISABLE_FULL_TEXT_SEARCH_SCENARIO,
    form_change_permission::DISABLE_FORM_CHANGE_SCENARIO,
    bsl_checkers::DUPLICATE_METHODS_SCENARIO,
    bsl_checkers::PREPROCESSOR_SCENARIO,
    bsl_checkers::REGIONS_SCENARIO,
    text_checkers::PROFANITY_SCENARIO,
    external_artifacts::EXTERNAL_ARTIFACTS_SCENARIO,
    metadata_sync::METADATA_SYNC_SCENARIO,
    composition_sort::COMPOSITION_SORT_SCENARIO,
    composition_sort::METADATA_TREE_SORT_SCENARIO,
    composition_sort::SUBSYSTEM_COMPOSITION_SORT_SCENARIO,
    duplicate_metadata::DUPLICATE_METADATA_SCENARIO,
    text_fixers::TRAILING_WHITESPACE_SCENARIO,
    text_fixers::EXTRA_BLANK_LINES_SCENARIO,
];

pub const REFERENCE_SCENARIO_METADATA: &[config::ScenarioMetadata] = &[
    text_fixers::COPYRIGHT_SCENARIO.metadata,
    text_fixers::KEYWORD_SPACING_SCENARIO.metadata,
    bsl_checkers::FORBID_GOTO_SCENARIO.metadata,
    text_fixers::CANONICAL_SPELLING_SCENARIO.metadata,
    xml_forms::XML_FORM_CORRECTION_SCENARIO.metadata,
    unit_tests_processing::UNIT_TESTS_PROCESSING_SCENARIO.metadata,
    full_text_search::DISABLE_FULL_TEXT_SEARCH_SCENARIO.metadata,
    form_change_permission::DISABLE_FORM_CHANGE_SCENARIO.metadata,
    bsl_checkers::DUPLICATE_METHODS_SCENARIO.metadata,
    bsl_checkers::PREPROCESSOR_SCENARIO.metadata,
    bsl_checkers::REGIONS_SCENARIO.metadata,
    text_checkers::PROFANITY_SCENARIO.metadata,
    UNSUPPORTED_ORDINARY_FORMS_METADATA,
    external_artifacts::EXTERNAL_ARTIFACTS_SCENARIO.metadata,
    metadata_sync::METADATA_SYNC_SCENARIO.metadata,
    composition_sort::COMPOSITION_SORT_SCENARIO.metadata,
    composition_sort::METADATA_TREE_SORT_SCENARIO.metadata,
    composition_sort::SUBSYSTEM_COMPOSITION_SORT_SCENARIO.metadata,
    duplicate_metadata::DUPLICATE_METADATA_SCENARIO.metadata,
    text_fixers::TRAILING_WHITESPACE_SCENARIO.metadata,
    text_fixers::EXTRA_BLANK_LINES_SCENARIO.metadata,
];

pub fn reference_scenario_catalog() -> config::ScenarioCatalog<'static> {
    config::ScenarioCatalog::new(REFERENCE_SCENARIO_METADATA)
}

pub fn reference_registry() -> scenario_pipeline::ScenarioRegistry {
    use scenario_pipeline::ScenarioRegistry;

    ScenarioRegistry::reference(reference_scenario_catalog())
        .with_definitions(REFERENCE_SCENARIO_DEFINITIONS.iter().copied())
}

#[cfg(test)]
mod tests {
    use crate::config::ScenarioSupport;
    use crate::scenarios::{find_reference_scenario, supported_reference_scenarios};

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
    fn reference_registry_binds_every_supported_reference_scenario_to_handler() {
        let registry = crate::reference_registry();

        for scenario in supported_reference_scenarios() {
            let registered = registry
                .get(scenario.id)
                .unwrap_or_else(|| panic!("scenario is not registered: {}", scenario.id));

            assert!(
                registered.has_registered_handler(),
                "scenario handler is not bound: {}",
                scenario.id
            );
        }
    }

    #[test]
    fn reference_scenario_inventory_matches_required_v1_list() {
        let required = crate::REFERENCE_SCENARIO_METADATA
            .iter()
            .filter(|scenario| scenario.support == ScenarioSupport::RequiredV1)
            .map(|scenario| scenario.id)
            .collect::<Vec<_>>();

        assert_eq!(required, EXPECTED_REQUIRED_V1);
    }

    #[test]
    fn reference_scenario_inventory_keeps_unsupported_ordinary_forms_explicit() {
        let scenario = find_reference_scenario("РазборОбычныхФормНаИсходники.os").unwrap();

        assert_eq!(scenario.id, crate::config::UNSUPPORTED_ORDINARY_FORMS);
        assert_eq!(scenario.support, ScenarioSupport::Unsupported);
    }

    #[test]
    fn reference_scenario_inventory_keeps_explicit_compatibility_scenarios_out_of_required_v1() {
        let compatibility = crate::REFERENCE_SCENARIO_METADATA
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
    fn reference_scenario_lookup_accepts_ids_with_and_without_os_suffix() {
        let plain = find_reference_scenario("УдалениеЛишнихКонцевыхПробелов").unwrap();
        let suffixed = find_reference_scenario("УдалениеЛишнихКонцевыхПробелов.os").unwrap();

        assert_eq!(plain, suffixed);
    }

    #[test]
    fn reference_scenario_facade_keeps_legacy_metadata_type_name() {
        let scenario: crate::scenarios::ScenarioDefinition =
            *find_reference_scenario("УдалениеЛишнихКонцевыхПробелов").unwrap();

        assert_eq!(scenario.id, "УдалениеЛишнихКонцевыхПробелов");
    }

    #[test]
    fn reference_scenario_inventory_fixture_preserves_reference_config_key_and_order() {
        let fixture = include_str!("../tests/fixtures/scenario_inventory/reference-v8config.json");

        assert!(fixture.contains("Precommt4onecСценарии"));
        assert!(fixture.contains("РазборОбычныхФормНаИсходники.os"));

        let mut previous_position = 0;
        for scenario in crate::REFERENCE_SCENARIO_METADATA
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
