pub mod app;
pub mod cli;

pub use prec_bsl_bsl::{
    bsl_checkers, bsl_parser, text_checkers, text_fixers, unit_tests_processing,
};
pub use prec_bsl_config as config;
pub use prec_bsl_git as git_index;
pub use prec_bsl_output as output;
pub use prec_bsl_pipeline as scenario_pipeline;
pub use prec_bsl_platform as external_artifacts;
pub use prec_bsl_scenarios as scenarios;
pub use prec_bsl_source as source_files;
pub use prec_bsl_xml::{
    composition_sort, duplicate_metadata, form_change_permission, full_text_search, metadata_sync,
    xml_edt, xml_forms,
};

pub fn reference_registry() -> scenario_pipeline::ScenarioRegistry {
    use bsl_checkers::{
        DUPLICATE_METHODS_RULE, FORBID_GOTO_RULE, PREPROCESSOR_RULE, REGIONS_RULE,
        duplicate_methods, forbid_goto, preprocessor_instructions, regions,
    };
    use composition_sort::{
        COMPOSITION_SORT_RULE, METADATA_TREE_SORT_RULE, SUBSYSTEM_COMPOSITION_SORT_RULE,
        composition_sort,
    };
    use duplicate_metadata::{DUPLICATE_METADATA_RULE, duplicate_metadata};
    use external_artifacts::{EXTERNAL_ARTIFACTS_RULE, external_artifacts};
    use form_change_permission::{DISABLE_FORM_CHANGE_RULE, disable_form_change_permission};
    use full_text_search::{DISABLE_FULL_TEXT_SEARCH_RULE, disable_full_text_search};
    use metadata_sync::{METADATA_SYNC_RULE, metadata_sync};
    use scenario_pipeline::ScenarioRegistry;
    use text_checkers::{PROFANITY_RULE, profanity};
    use text_fixers::{
        CANONICAL_SPELLING_RULE, COPYRIGHT_RULE, EXTRA_BLANK_LINES_RULE, KEYWORD_SPACING_RULE,
        TRAILING_WHITESPACE_RULE, canonical_spelling, copyright, extra_blank_lines,
        keyword_spacing, trailing_whitespace,
    };
    use unit_tests_processing::{UNIT_TESTS_PROCESSING_RULE, unit_tests_processing};
    use xml_forms::{XML_FORM_CORRECTION_RULE, xml_form_correction};

    ScenarioRegistry::reference()
        .with_handler(COPYRIGHT_RULE, copyright)
        .with_handler(TRAILING_WHITESPACE_RULE, trailing_whitespace)
        .with_handler(EXTRA_BLANK_LINES_RULE, extra_blank_lines)
        .with_handler(KEYWORD_SPACING_RULE, keyword_spacing)
        .with_handler(CANONICAL_SPELLING_RULE, canonical_spelling)
        .with_handler(FORBID_GOTO_RULE, forbid_goto)
        .with_handler(DUPLICATE_METHODS_RULE, duplicate_methods)
        .with_handler(PREPROCESSOR_RULE, preprocessor_instructions)
        .with_handler(REGIONS_RULE, regions)
        .with_handler(PROFANITY_RULE, profanity)
        .with_handler(UNIT_TESTS_PROCESSING_RULE, unit_tests_processing)
        .with_handler(XML_FORM_CORRECTION_RULE, xml_form_correction)
        .with_handler(DISABLE_FULL_TEXT_SEARCH_RULE, disable_full_text_search)
        .with_handler(DISABLE_FORM_CHANGE_RULE, disable_form_change_permission)
        .with_deleted_file_handler(METADATA_SYNC_RULE, metadata_sync)
        .with_handler(COMPOSITION_SORT_RULE, composition_sort)
        .with_handler(METADATA_TREE_SORT_RULE, composition_sort)
        .with_handler(SUBSYSTEM_COMPOSITION_SORT_RULE, composition_sort)
        .with_handler(DUPLICATE_METADATA_RULE, duplicate_metadata)
        .with_handler(EXTERNAL_ARTIFACTS_RULE, external_artifacts)
}
