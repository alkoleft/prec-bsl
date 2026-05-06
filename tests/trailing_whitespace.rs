use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioRegistry, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{TRAILING_WHITESPACE_RULE, remove_trailing_spaces_and_tabs};

#[test]
fn trailing_whitespace_matches_golden_fixture_and_is_idempotent() {
    let repo = temp_repo("golden");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/УдалениеЛишнихКонцевыхПробелов/кириллический_модуль/input.bsl",
    );
    let expected = read_fixture_text(
        "tests/fixtures/golden/УдалениеЛишнихКонцевыхПробелов/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = trailing_whitespace_config();

    let first_report = run_pipeline(
        &ScenarioRegistry::reference(),
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
    assert_eq!(first_report.results[0].rule_id, TRAILING_WHITESPACE_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(
        first_report.results[0].message,
        "removed trailing spaces or tabs"
    );
    assert_eq!(first_report.modified_paths(), vec![repo_path.clone()]);
    assert_eq!(
        first_report.hook_exit_code(),
        1,
        "hook mode must block after unreviewed fixer modifications"
    );

    let second_report = run_pipeline(
        &ScenarioRegistry::reference(),
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
fn trailing_whitespace_preserves_line_endings() {
    let input = "Процедура A()   \n\tСообщить();\t\r\nКонецПроцедуры  ";
    let output = remove_trailing_spaces_and_tabs(input);

    assert_eq!(output, "Процедура A()\n\tСообщить();\r\nКонецПроцедуры");
}

fn trailing_whitespace_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
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
        .join("trailing-whitespace-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary trailing whitespace test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
