//! Parity-приёмка `ПроверкаПереносовСтрок` на контрольном корпусе yaxunit
//! (issue #5): прогон детектора `find_line_break_violations` на всех `.bsl`
//! под `exts`, `tests`, `fixtures` корпуса и сравнение с закоммиченным
//! baseline `tests/fixtures/parity/ilb-fingerprint.txt`.
//!
//! Корпус read-only: тест только читает файлы (никаких записей в корпус).
//!
//! Путь корпуса задаётся env `YAXUNIT_CORPUS` (default
//! `/data/home/korav/projects/yaxunit`). Если каталог отсутствует, тест
//! молча пропускается с eprintln-сообщением (паттерн `rat_acceptance.rs`) —
//! на CI без корпуса тест не падает, на машине с корпусом гоняется.
//!
//! Регенерация baseline описана в шапке baseline-файла.

use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_YAXUNIT_CORPUS: &str = "/data/home/korav/projects/yaxunit";
const BASELINE_PATH: &str = "tests/fixtures/parity/ilb-fingerprint.txt";

fn resolved_corpus_path() -> PathBuf {
    std::env::var_os("YAXUNIT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_YAXUNIT_CORPUS))
}

fn yaxunit_corpus() -> Option<PathBuf> {
    let path = resolved_corpus_path();
    path.join("exts").is_dir().then_some(path)
}

/// Рекурсивный сбор `.bsl`-файлов (read-only обход) в детерминированном
/// порядке: сначала отсортированный список файлов, находки внутри файла —
/// по порядку строк.
fn collect_bsl_files(root: &Path, files: &mut Vec<PathBuf>) -> Option<()> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            collect_bsl_files(&path, files);
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("bsl")
        {
            files.push(path);
        }
    }
    files.sort();
    Some(())
}

/// Fingerprint-строки текущего прогона: `path|rule|message` на каждую находку,
/// пути — относительные от корня корпуса, разделители нормализованы в `/`.
fn collect_current_fingerprint(corpus: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for root in ["exts", "tests", "fixtures"] {
        collect_bsl_files(&corpus.join(root), &mut files);
    }

    let settings = prec_bsl::line_break::LineBreakSettings::default_settings();
    let mut fingerprint = Vec::new();
    for file in &files {
        let input = fs::read_to_string(file).unwrap_or_else(|error| {
            panic!("failed to read corpus file {}: {error}", file.display())
        });
        let violations = prec_bsl::line_break::find_line_break_violations(&input, &settings);
        let relative = file
            .strip_prefix(corpus)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        for violation in &violations {
            fingerprint.push(format!(
                "{relative}|ПроверкаПереносовСтрок|incorrect line break at line {}",
                violation.line
            ));
        }
    }
    fingerprint
}

/// Строки baseline без комментариев (`#`-префикс) и пустых строк.
fn baseline_fingerprint_lines() -> Vec<String> {
    let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE_PATH);
    let baseline = fs::read_to_string(&baseline_path).unwrap_or_else(|error| {
        panic!(
            "failed to read parity baseline {}: {error}",
            baseline_path.display()
        )
    });

    baseline
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn yaxunit_line_break_parity_matches_committed_baseline() {
    let Some(corpus) = yaxunit_corpus() else {
        eprintln!(
            "skipping yaxunit line-break parity: corpus {} is not available; \
             set YAXUNIT_CORPUS to run",
            resolved_corpus_path().display()
        );
        return;
    };

    let baseline = baseline_fingerprint_lines();
    assert!(
        !baseline.is_empty(),
        "parity baseline must contain at least one fingerprint line"
    );

    let current = collect_current_fingerprint(&corpus);

    // Полная эквивалентность последовательностей (не только счётчик): любые
    // новые/пропавшие находки и сдвиги номеров строк — падение теста.
    assert_eq!(
        current.len(),
        baseline.len(),
        "line-break parity finding count changed: baseline {} findings vs current {}",
        baseline.len(),
        current.len()
    );

    let mut first_difference = None;
    for (index, (current_line, baseline_line)) in current.iter().zip(baseline.iter()).enumerate() {
        if current_line != baseline_line {
            first_difference = Some((index, current_line.clone(), baseline_line.clone()));
            break;
        }
    }

    if let Some((index, current_line, baseline_line)) = first_difference {
        // Дополнительно показываем множественную разницу для читаемого отчёта.
        let missing: Vec<_> = baseline
            .iter()
            .filter(|line| !current.contains(line))
            .cloned()
            .collect();
        let added: Vec<_> = current
            .iter()
            .filter(|line| !baseline.contains(line))
            .cloned()
            .collect();
        panic!(
            "line-break parity fingerprint diverged at entry {index}:\n  \
             baseline: {baseline_line}\n  current:  {current_line}\n\
             findings only in baseline ({}): {missing:?}\n\
             findings only in current ({}): {added:?}",
            missing.len(),
            added.len()
        );
    }
}
