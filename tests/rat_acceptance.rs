#[path = "support/rat.rs"]
mod rat_support;

use std::fs;
use std::path::Path;

use prec_bsl::scenarios::{ScenarioSupport, find_reference_scenario, normalize_scenario_id};
use rat_support::{
    RAT_SOURCE_ROOTS, TempRatCopy, collect_source_files, copy_required_source_roots,
    git_status_short, rat_repo,
};
use serde_json::Value;

#[test]
fn rat_source_roots_copy_to_tempdir_without_mutating_checkout() {
    let Some(repo) = rat_repo() else {
        eprintln!("skipping RAT acceptance: /home/alko/develop/open-source/rat is not available");
        return;
    };

    let before_status = git_status_short(repo).expect("RAT git status must be readable");
    let tempdir = TempRatCopy::new().expect("temporary RAT copy directory must be created");

    let copied_roots = copy_required_source_roots(repo, tempdir.path())
        .expect("required RAT source roots must copy into a temporary directory");

    assert_eq!(copied_roots.len(), RAT_SOURCE_ROOTS.len());
    for root in &copied_roots {
        assert!(
            root.is_dir(),
            "copied RAT source root must exist: {}",
            root.display()
        );
    }

    let copied_files = copied_roots
        .iter()
        .map(|root| collect_source_files(root))
        .collect::<Result<Vec<_>, _>>()
        .expect("copied RAT source files must be discoverable")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    assert!(
        copied_files.iter().any(|path| has_extension(path, "bsl")),
        "temporary RAT copy must include BSL modules"
    );
    assert!(
        copied_files.iter().any(|path| has_extension(path, "mdo")),
        "temporary RAT copy must include EDT metadata"
    );
    assert!(
        copied_files
            .iter()
            .any(|path| path.to_string_lossy().chars().any(|char| !char.is_ascii())),
        "temporary RAT copy must preserve Cyrillic paths"
    );

    fs::write(
        tempdir
            .path()
            .join("fixtures/configuration/prec-bsl-temp-marker.txt"),
        "mutating acceptance checks write only to the temp copy\n",
    )
    .expect("temporary RAT copy must be writable");

    assert!(
        !repo
            .join("fixtures/configuration/prec-bsl-temp-marker.txt")
            .exists(),
        "acceptance test must not write marker files into the real RAT checkout"
    );

    let after_status = git_status_short(repo).expect("RAT git status must be readable after copy");
    assert_eq!(
        after_status, before_status,
        "copy-only RAT acceptance must not change real checkout status"
    );
}

#[test]
fn rat_live_v8config_parses_and_reports_repository_local_scenarios() {
    let Some(repo) = rat_repo() else {
        eprintln!("skipping RAT acceptance: /home/alko/develop/open-source/rat is not available");
        return;
    };

    let config_path = repo.join("v8config.json");
    let config = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", config_path.display()));
    let config: Value = serde_json::from_str(&config)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", config_path.display()));

    assert_eq!(
        config["GLOBAL"]["ФорматEDT"],
        Value::Bool(true),
        "RAT live config must preserve the EDT-format compatibility fixture"
    );
    assert_eq!(
        config["GLOBAL"]["ВерсияПлатформы"],
        Value::String("8.3.20.1996".to_owned())
    );

    let scenario_config = config
        .get("Precommt4onecСценарии")
        .expect("historic Precommt4onecСценарии key must parse");
    let enabled = scenario_strings(scenario_config, "ГлобальныеСценарии");
    let disabled = scenario_strings(scenario_config, "ОтключенныеСценарии");

    assert!(
        enabled
            .iter()
            .any(|scenario| *scenario == "УдалениеЛишнихКонцевыхПробелов.os"),
        "RAT live config must exercise reference global scenarios"
    );
    assert!(
        disabled
            .iter()
            .any(|scenario| *scenario == "РазборОбычныхФормНаИсходники"),
        "disabled unsupported built-in scenario must remain parse-compatible"
    );

    let diagnostics = enabled
        .iter()
        .filter_map(|scenario| unsupported_enabled_scenario_diagnostic(scenario))
        .collect::<Vec<_>>();

    assert_eq!(
        diagnostics,
        vec![
            "unsupported repository-local scenario in v1: СортировкаДереваМетаданных; dynamic local .os execution is not supported in v1".to_owned(),
            "unsupported repository-local scenario in v1: СортировкаСоставаПодсистем; dynamic local .os execution is not supported in v1".to_owned(),
        ]
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.contains("РазборОбычныхФормНаИсходники")),
        "disabled unsupported scenarios must not be reported as enabled failures"
    );
}

fn scenario_strings<'a>(config: &'a Value, key: &str) -> Vec<&'a str> {
    config
        .get(key)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{key} must be an array"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{key} must contain only strings"))
        })
        .collect()
}

fn unsupported_enabled_scenario_diagnostic(scenario: &str) -> Option<String> {
    let normalized = normalize_scenario_id(scenario);
    match find_reference_scenario(scenario).map(|definition| definition.support) {
        Some(ScenarioSupport::RequiredV1) => None,
        Some(ScenarioSupport::Unsupported) => {
            Some(format!("unsupported built-in scenario in v1: {normalized}"))
        }
        None => Some(format!(
            "unsupported repository-local scenario in v1: {normalized}; dynamic local .os execution is not supported in v1"
        )),
    }
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == extension)
}
