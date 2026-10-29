mod support;

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{EXTRA_BLANK_LINES_RULE, remove_extra_blank_lines};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use support::fixtures::{
    FixtureDiagnostic, FixtureRun, FixtureSeverity, GoldenFixture, assert_fixer_fixture,
};

#[test]
fn empty_lines_matches_golden_fixture_and_is_idempotent() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/УдалениеЛишнихПустыхСтрок/кириллический_модуль",
        PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl"),
    );

    assert_fixer_fixture(&fixture, |input, path| {
        empty_lines_pipeline_run("cyrillic", input, path)
    });
}

#[test]
fn empty_lines_matches_trailing_blank_run_golden_fixture_and_is_idempotent() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/УдалениеЛишнихПустыхСтрок/хвостовой_пробельный_блок",
        PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl"),
    );

    assert_fixer_fixture(&fixture, |input, path| {
        empty_lines_pipeline_run("trailing", input, path)
    });
}

fn empty_lines_pipeline_run(test_name: &str, input: &str, path: &Path) -> FixtureRun {
    let repo = temp_repo(test_name);
    write_file(repo.join(path), input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, path.to_path_buf(), None).unwrap();
    let config = empty_lines_config();

    let report = run_pipeline(
        &prec_bsl::reference_registry(),
        PipelineRequest {
            repo_root: &repo,
            source_roots: &roots,
            config: &config,
            files: vec![file],
            mode: PipelineMode::Hook,
        },
    );

    let output = fs::read_to_string(repo.join(path)).unwrap();
    let diagnostics: Vec<FixtureDiagnostic> = report
        .results
        .iter()
        .map(|result| {
            assert_eq!(result.status, ScenarioResultStatus::Modified);
            assert_eq!(result.rule_id, EXTRA_BLANK_LINES_RULE);
            FixtureDiagnostic::new(
                FixtureSeverity::Modified,
                result.rule_id.clone(),
                path,
                result.message.clone(),
            )
        })
        .collect();

    // AGENTS.md modeling rule: hook mode exits 0 only when no blocking
    // diagnostics and no unreviewed modifications remain.
    let expected_exit_code = if diagnostics.is_empty() { 0 } else { 1 };
    assert_eq!(
        report.hook_exit_code(),
        expected_exit_code,
        "hook exit code must block on unreviewed fixer modifications"
    );
    assert_eq!(
        report.modified_paths(),
        if diagnostics.is_empty() {
            Vec::new()
        } else {
            vec![path.to_path_buf()]
        },
        "modified paths must report every silently changed file"
    );

    FixtureRun::new(output, diagnostics)
}

#[test]
fn empty_lines_preserves_meaningful_spacing_and_line_endings() {
    let input = "Процедура A()\r\n\r\n\r\n\tСообщить();\r\n \t\r\nКонецПроцедуры()";
    let output = remove_extra_blank_lines(input);

    assert_eq!(
        output,
        "Процедура A()\r\n\r\n\tСообщить();\r\n \t\r\nКонецПроцедуры()"
    );
}

#[test]
fn empty_lines_line_ending_matrix_is_idempotent() {
    for (name, line_ending) in [("lf", "\n"), ("crlf", "\r\n")] {
        let input = format!(
            "Процедура A(){line_ending}{line_ending}{line_ending}\tСообщить();{line_ending}КонецПроцедуры{line_ending}"
        );
        let expected = format!(
            "Процедура A(){line_ending}{line_ending}\tСообщить();{line_ending}КонецПроцедуры{line_ending}"
        );

        let first = remove_extra_blank_lines(&input);
        let second = remove_extra_blank_lines(&first);

        assert_eq!(first, expected, "{name} rewrite must preserve line endings");
        assert_eq!(second, expected, "{name} second rewrite must be clean");
    }
}

#[test]
fn empty_lines_file_of_only_blank_lines_becomes_empty() {
    for input in ["\n", "\n\n\n", " \t\n\n", "\r\n\r\n"] {
        let output = remove_extra_blank_lines(input);
        assert_eq!(
            output, "",
            "a file consisting only of blank lines must become empty"
        );
        assert_eq!(
            remove_extra_blank_lines(&output),
            "",
            "empty output must stay empty"
        );
    }
}

#[test]
fn empty_lines_file_without_final_newline_is_untouched() {
    let input = "Процедура A()\n\nКонецПроцедуры";
    let output = remove_extra_blank_lines(input);

    assert_eq!(
        output, input,
        "a final line without a line ending must not gain or lose blank lines"
    );
}

fn empty_lines_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
            }
        }"#,
    )
    .unwrap()
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("empty-lines-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary empty lines test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
