use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::config::parse_config_str;
use prec_bsl::scenario_pipeline::{
    PipelineMode, PipelineRequest, ScenarioResultStatus, run_pipeline,
};
use prec_bsl::source_files::{classify_repo_path, resolve_source_roots};
use prec_bsl::text_fixers::{COMMA_SPACING_RULE, fix_comma_spacing};

#[test]
fn comma_spacing_matches_golden_fixture_and_is_idempotent() {
    let repo = temp_repo("golden");
    let repo_path = PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl");
    let input = read_fixture_text(
        "tests/fixtures/golden/ПробелыПослеЗапятых/кириллический_модуль/input.bsl",
    );
    let expected = read_fixture_text(
        "tests/fixtures/golden/ПробелыПослеЗапятых/кириллический_модуль/expected.bsl",
    );
    write_file(repo.join(&repo_path), &input);

    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
    let file = classify_repo_path(&roots, repo_path.clone(), None).unwrap();
    let config = comma_spacing_config();

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
    assert_eq!(first_report.results[0].rule_id, COMMA_SPACING_RULE);
    assert_eq!(
        first_report.results[0].status,
        ScenarioResultStatus::Modified
    );
    assert_eq!(first_report.results[0].message, "added spaces after commas");
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
fn comma_spacing_skips_strings_comments_multiline_literals_and_line_breaks() {
    let input = concat!(
        "Строка = \"запятая,внутри строки\";\n",
        "Строка2 = \"текст \"\"а,б\"\" кавычки\";\n",
        "// комментарий,без пробела\n",
        "Текст = \"первая,строка\n",
        "|вторая,строка литерала\n",
        "|\";\n",
        "Метод1(24,\n",
        "\t60,\n",
        "\t60);\n",
        "Метод2(60, 60, 60);\n",
        "Метод3(А,Б);\n",
    );
    let output = fix_comma_spacing(input);

    assert_eq!(
        output,
        concat!(
            "Строка = \"запятая,внутри строки\";\n",
            "Строка2 = \"текст \"\"а,б\"\" кавычки\";\n",
            "// комментарий,без пробела\n",
            "Текст = \"первая,строка\n",
            "|вторая,строка литерала\n",
            "|\";\n",
            "Метод1(24,\n",
            "\t60,\n",
            "\t60);\n",
            "Метод2(60, 60, 60);\n",
            "Метод3(А, Б);\n",
        )
    );
}

#[test]
fn comma_spacing_line_ending_matrix_is_idempotent() {
    for (name, line_ending) in [("lf", "\n"), ("crlf", "\r\n")] {
        let input = format!(
            "Метод(А,Б){line_ending}Метод2(В, Г){line_ending}Метод3(Д,{line_ending}\tЕ);{line_ending}"
        );
        let expected = format!(
            "Метод(А, Б){line_ending}Метод2(В, Г){line_ending}Метод3(Д,{line_ending}\tЕ);{line_ending}"
        );

        let first = fix_comma_spacing(&input);
        let second = fix_comma_spacing(&first);

        assert_eq!(first, expected, "{name} rewrite must preserve line endings");
        assert_eq!(second, expected, "{name} second rewrite must be clean");
    }
}

fn comma_spacing_config() -> prec_bsl::config::ResolvedConfig {
    parse_config_str(
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ПробелыПослеЗапятых.os"]
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
        .join("comma-spacing-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary comma spacing test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}
