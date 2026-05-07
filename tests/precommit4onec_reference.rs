use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const REFERENCE_ROOT: &str = "tests/fixtures/precommit4onec-reference";
const EXPECTED_FILE_COUNT: usize = 531;
const REFERENCE_TEST_MODULES: &[&str] = &[
    "ТестВыполнениеСценариев.os",
    "ТестНастройкиРепозитория.os",
    "ТестПроверкаСценариевОбработки.os",
    "ТестРедакторНастроек.os",
    "ТестФайловыеОперации.os",
];

#[test]
fn precommit4onec_reference_test_cases_are_imported() {
    let root = Path::new(REFERENCE_ROOT);
    assert!(
        root.is_dir(),
        "precommit4onec reference test cases must be present"
    );

    let files = collect_files(root);
    assert_eq!(
        files.len(),
        EXPECTED_FILE_COUNT,
        "reference corpus file count changed; update the imported corpus deliberately"
    );

    for path in [
        "ТестВыполнениеСценариев.os",
        "ТестНастройкиРепозитория.os",
        "ТестПроверкаСценариевОбработки.os",
        "ТестРедакторНастроек.os",
        "ТестФайловыеОперации.os",
        "fixtures/v8config.json",
        "fixtures/ИсправлениеНеКаноническогоНаписания.bsl",
        "fixtures/ПроверкаДублейПроцедурНегативныйТест.bsl",
        "fixtures/cf-edt/configuration/src/Configuration/Configuration.mdo",
        "fixtures/cf-common-forms/src/Configuration.xml",
        "fixtures/ВставкаКопирайтов/COPYRIGHT",
        "fixtures/СортировкаСостава/Configuration/До/Configuration.mdo",
        "fixtures/СортировкаПравРолей/До/EDT/Администратор/Rights.rights",
    ] {
        assert!(
            root.join(path).is_file(),
            "expected reference test case file must exist: {path}"
        );
    }

    for path in [
        "fixtures/ВставкаКопирайтов",
        "fixtures/ВыполнениеСценариев",
        "fixtures/ЗапретИспользованияПерейти",
        "fixtures/КорректировкаXMLФорм",
        "fixtures/ПроверкаКорректностиИнструкцийПрепроцессора",
        "fixtures/ПроверкаНецензурныхСлов",
        "fixtures/СинхронизацияОбъектовМетаданныхИФайлов",
        "fixtures/СортировкаПравРолей",
        "fixtures/СортировкаСостава",
        "fixtures/ХранениеРазныхНастроек",
    ] {
        assert!(
            root.join(path).is_dir(),
            "expected reference fixture directory must exist: {path}"
        );
    }
}

#[test]
fn precommit4onec_reference_executable_cases_are_mapped_in_testing_strategy() {
    let testing_strategy =
        fs::read_to_string("spec/testing-strategy.md").expect("testing strategy must be readable");

    for module in REFERENCE_TEST_MODULES {
        let module_text = fs::read_to_string(Path::new(REFERENCE_ROOT).join(module))
            .unwrap_or_else(|error| panic!("failed to read {module}: {error}"));

        for test_case in extract_reference_test_cases(&module_text) {
            assert!(
                has_reference_mapping_row(&testing_strategy, &test_case),
                "reference executable test case is not mapped in spec/testing-strategy.md: {module}::{test_case}"
            );
        }
    }
}

#[test]
fn precommit4onec_reference_json_fixtures_are_valid_json() {
    let json_files = collect_files(Path::new(REFERENCE_ROOT))
        .into_iter()
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();

    assert!(!json_files.is_empty(), "reference corpus must include JSON");

    for path in json_files {
        let relative_path = relative_reference_path(&path);
        if is_known_non_json_reference_fixture(relative_path) {
            continue;
        }

        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        serde_json::from_str::<serde_json::Value>(&content)
            .unwrap_or_else(|error| panic!("invalid JSON in {}: {error}", path.display()));
    }
}

#[test]
fn precommit4onec_reference_text_fixtures_are_utf8() {
    let mut counts_by_extension = BTreeMap::new();

    for path in collect_files(Path::new(REFERENCE_ROOT)) {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("<none>");

        if !is_reference_text_extension(extension) {
            continue;
        }

        let relative_path = relative_reference_path(&path);
        if is_known_binary_reference_fixture(relative_path) {
            continue;
        }

        fs::read_to_string(&path).unwrap_or_else(|error| {
            panic!(
                "reference text fixture is not UTF-8 {}: {error}",
                path.display()
            )
        });
        *counts_by_extension
            .entry(extension.to_owned())
            .or_insert(0usize) += 1;
    }

    assert_eq!(counts_by_extension.get("bsl"), Some(&121));
    assert_eq!(counts_by_extension.get("os"), Some(&8));
    assert_eq!(counts_by_extension.get("xml"), Some(&168));
    assert_eq!(counts_by_extension.get("mdo"), Some(&127));
    assert_eq!(counts_by_extension.get("form"), Some(&23));
    assert_eq!(counts_by_extension.get("json"), Some(&32));
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files);
    files.sort();
    files
}

fn collect_files_recursive(path: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
    {
        let entry = entry.unwrap_or_else(|error| {
            panic!(
                "failed to read directory entry in {}: {error}",
                path.display()
            )
        });
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn extract_reference_test_cases(module_text: &str) -> Vec<String> {
    module_text
        .lines()
        .filter_map(|line| {
            let start = line.find("ВсеТесты.Добавить(\"")? + "ВсеТесты.Добавить(\"".len();
            let rest = &line[start..];
            let end = rest.find('"')?;
            Some(rest[..end].to_owned())
        })
        .collect()
}

fn has_reference_mapping_row(testing_strategy: &str, test_case: &str) -> bool {
    let prefix = format!("| `{test_case}` | `");
    testing_strategy.lines().any(|line| {
        line.starts_with(&prefix)
            && (line.contains("`covered`:")
                || line.contains("`blocked`:")
                || line.contains("`out-of-scope`:"))
    })
}

fn relative_reference_path(path: &Path) -> &Path {
    path.strip_prefix(REFERENCE_ROOT)
        .unwrap_or_else(|error| panic!("reference path must be under corpus root: {error}"))
}

fn is_known_non_json_reference_fixture(path: &Path) -> bool {
    path == Path::new("fixtures/ПроверкаСообщенияКоммита/v8config.json")
}

fn is_known_binary_reference_fixture(path: &Path) -> bool {
    path == Path::new("fixtures/ЗащищенныеФайлы/Module.bsl")
        || path
            == Path::new(
                "fixtures/СинхронизацияОбъектовМетаданныхИФайлов/EDT/src/CommonForms/ФормаКонстант/Form.oform",
            )
}

fn is_reference_text_extension(extension: &str) -> bool {
    matches!(
        extension,
        "<none>"
            | "PMF"
            | "bsl"
            | "cmi"
            | "dcs"
            | "dcss"
            | "form"
            | "json"
            | "mdo"
            | "oform"
            | "os"
            | "prefs"
            | "project"
            | "rights"
            | "txt"
            | "xml"
    )
}
