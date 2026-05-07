use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{CANONICAL_SPELLING_RULE, fix_non_canonical_spelling};

const REFERENCE_ROOT: &str = "tests/fixtures/precommit4onec-reference";

#[test]
fn canonical_spelling_matches_golden_fixture_and_is_idempotent() {
    let repo = temp_repo("golden");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/ИсправлениеНеКаноническогоНаписания/кириллический_модуль/input.bsl",
    );
    let expected = read_fixture_text(
        "tests/fixtures/golden/ИсправлениеНеКаноническогоНаписания/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = canonical_spelling_config();

    let first_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file.clone()],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), expected);
    assert_eq!(first_report.results.len(), 1);
    assert_eq!(first_report.results[0].rule_id, CANONICAL_SPELLING_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        first_report.results[0].message,
        "fixed non-canonical keyword spelling"
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(
        first_report.hook_exit_code(),
        1,
        "hook mode must block after unreviewed fixer modifications"
    );

    let second_report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert!(second_report.results.is_empty());
    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), expected);
    assert_eq!(second_report.hook_exit_code(), 0);
}

#[test]
fn canonical_spelling_skips_comments_strings_and_identifier_parts() {
    let input = concat!(
        "// если null\n",
        "Сообщить(\"если null\");\n",
        "Текст = \"первая\n",
        "|возврат null\n",
        "|\";\n",
        "ЕслиФлаг = Истина;\n",
        "если Флаг тогда\n",
        "\tвозврат ложь;\n",
        "конецесли;\n",
    );
    let output = fix_non_canonical_spelling(input);

    assert_eq!(
        output,
        concat!(
            "// если null\n",
            "Сообщить(\"если null\");\n",
            "Текст = \"первая\n",
            "|возврат null\n",
            "|\";\n",
            "ЕслиФлаг = Истина;\n",
            "Если Флаг Тогда\n",
            "\tВозврат Ложь;\n",
            "КонецЕсли;\n",
        )
    );
}

#[test]
fn canonical_spelling_skips_reference_extension_modules_with_change_control() {
    let repo = temp_repo("reference_change_control");
    let repo_path = PathBuf::from("src/МодулиРасширения/МодульСКонтролемИзменений.bsl");
    let input = fs::read_to_string(
        Path::new(REFERENCE_ROOT).join("fixtures/МодулиРасширения/МодульСКонтролемИзменений.bsl"),
    )
    .unwrap();
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &canonical_spelling_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert!(report.results.is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

#[test]
fn canonical_spelling_skips_change_control_marker_case_insensitively_anywhere_in_file() {
    let repo = temp_repo("change_control_case_insensitive");
    let repo_path = PathBuf::from("src/Module.bsl");
    let input = concat!(
        "Процедура ПередМаркером()\n",
        "конецпроцедуры\n",
        "\n",
        "    &изменениеиконтроль(\"ПроцедураСНеканоническимНаписанием\")\n",
        "процедура ПроцедураСНеканоническимНаписанием()\n",
        "конецпроцедуры\n",
    );
    write_file(repo.join(&repo_path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &canonical_spelling_config(),
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    assert_eq!(fs::read_to_string(repo.join(&repo_path)).unwrap(), input);
    assert!(report.results.is_empty());
    assert_eq!(report.hook_exit_code(), 0);
}

fn canonical_spelling_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ИсправлениеНеКаноническогоНаписания.os"]
            }
        }"#,
    )
    .unwrap()
}

fn read_fixture_text(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .unwrap()
        .replace("<SP>", " ")
        .replace("<TAB>", "\t")
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("canonical-spelling-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary canonical spelling test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
