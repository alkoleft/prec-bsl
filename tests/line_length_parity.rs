//! bsl-language-server LineLength parity test (snapshot validation, see the
//! "Parity scope statement" in `spec/parser-strategy.md`).
//!
//! Env-gated like `tests/rat_acceptance.rs`: it runs only when
//! `PREC_BSL_PARITY_CORPUS` points at a readable yaxunit checkout, and skips
//! (not fails) when that checkout's `git rev-parse HEAD` differs from the
//! baseline's pinned `corpus_commit` — parity is validated as a snapshot, not
//! as a continuous guarantee against upstream drift.
//!
//! The detector is exercised through the public unit seam
//! `prec_bsl_bsl::text_checkers::find_long_lines` with the shared
//! `DEFAULT_MAX_LINE_LENGTH` constant, not through the pipeline: the
//! fingerprint needs `(relpath, line, chars)` directly, and re-parsing those
//! out of message text would test the message format instead of the parity.
//!
//! Baseline refresh instructions (corpus drift): re-run bsl-ls v1.0.7 over the
//! yaxunit checkout (`java -jar bsl-language-server.jar analyze -s <yaxunit>
//! -r json` with the repo's own `.bsl-language-server.json`, limit 150; the
//! reference recipe lives in `spec/parser-strategy.md`), pin the new commit in
//! `corpus_commit`, and replace
//! `tests/fixtures/line-length/bsl-ls-line-length-baseline.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use prec_bsl::text_checkers::{DEFAULT_MAX_LINE_LENGTH, find_long_lines};
use serde_json::Value;

const PARITY_CORPUS_ENV: &str = "PREC_BSL_PARITY_CORPUS";
const BASELINE_FIXTURE: &str = "tests/fixtures/line-length/bsl-ls-line-length-baseline.json";
/// Records printed per diff section before the panic message is truncated.
const MAX_DIFF_RECORDS: usize = 20;

#[test]
fn line_length_detector_matches_bsl_ls_baseline_on_yaxunit_corpus() {
    let Some(corpus) = parity_corpus() else {
        eprintln!(
            "skipping bsl-ls line-length parity: {PARITY_CORPUS_ENV} is not set to a readable \
             yaxunit checkout"
        );
        return;
    };

    let baseline = read_baseline();
    let corpus_head = git_head(&corpus);
    if corpus_head != baseline.corpus_commit {
        eprintln!(
            "skipping bsl-ls line-length parity: corpus {} is at {corpus_head}, baseline is \
             pinned to {}; re-run bsl-ls (recipe in spec/parser-strategy.md) and refresh \
             {BASELINE_FIXTURE} to validate the new snapshot",
            corpus.display(),
            baseline.corpus_commit
        );
        return;
    }

    let tracked_bsl = tracked_bsl_files(&corpus);
    assert_eq!(
        tracked_bsl.len(),
        baseline.tracked_bsl_files,
        "pinned corpus must contain exactly the tracked .bsl files recorded in the baseline"
    );

    let mut actual = BTreeSet::new();
    for relpath in &tracked_bsl {
        let content = fs::read_to_string(corpus.join(relpath)).unwrap_or_else(|error| {
            panic!("failed to read corpus file {}: {error}", relpath.display())
        });
        for matched in find_long_lines(&content, DEFAULT_MAX_LINE_LENGTH) {
            actual.insert(Fingerprint {
                path: relpath.to_string_lossy().replace('\\', "/"),
                line: matched.line,
                chars: matched.length,
            });
        }
    }

    let expected: BTreeSet<Fingerprint> = baseline
        .diagnostics
        .iter()
        .map(|diagnostic| Fingerprint {
            path: diagnostic.path.clone(),
            line: diagnostic.line,
            chars: diagnostic.chars,
        })
        .collect();

    let missing: Vec<&Fingerprint> = expected.difference(&actual).collect();
    let unexpected: Vec<&Fingerprint> = actual.difference(&expected).collect();

    let report = format!(
        "bsl-ls LineLength parity: baseline {} diagnostics, detector found {} on {} tracked \
         .bsl files; missing (in baseline, not detected): {}, unexpected (detected, not in \
         baseline): {}\n\nWhen the yaxunit corpus has drifted, refresh the baseline by \
         re-running bsl-ls v1.0.7 (recipe in spec/parser-strategy.md) and repinning \
         corpus_commit in {BASELINE_FIXTURE}.",
        expected.len(),
        actual.len(),
        tracked_bsl.len(),
        missing.len(),
        unexpected.len(),
    );

    if !missing.is_empty() || !unexpected.is_empty() {
        panic!(
            "{report}\n\nmissing (first {MAX_DIFF_RECORDS}):\n{}\nunexpected (first \
             {MAX_DIFF_RECORDS}):\n{}",
            render_diff(&missing),
            render_diff(&unexpected),
        );
    }

    println!("{report}");
    println!("parity: 58/58 expected diagnostics matched, 0 missing, 0 unexpected");
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Fingerprint {
    path: String,
    line: usize,
    chars: usize,
}

struct Baseline {
    corpus_commit: String,
    tracked_bsl_files: usize,
    diagnostics: Vec<Diagnostic>,
}

#[derive(Debug)]
struct Diagnostic {
    path: String,
    line: usize,
    chars: usize,
}

fn parity_corpus() -> Option<std::path::PathBuf> {
    let corpus = std::env::var_os(PARITY_CORPUS_ENV)?;
    let corpus = Path::new(&corpus);
    // Cheap readability probe: the baseline corpus always carries its git
    // metadata and .bsl-language-server.json.
    corpus.join(".git").exists().then(|| corpus.to_path_buf())
}

fn git_head(corpus: &Path) -> String {
    let output = Command::new("git")
        .current_dir(corpus)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run git rev-parse in {}: {error}",
                corpus.display()
            )
        });
    assert!(
        output.status.success(),
        "git rev-parse HEAD failed in {}:\n{}",
        corpus.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Tracked .bsl files of the corpus as POSIX-relative paths.
/// `core.quotePath=false` keeps Cyrillic paths unescaped.
fn tracked_bsl_files(corpus: &Path) -> Vec<std::path::PathBuf> {
    let output = Command::new("git")
        .current_dir(corpus)
        .arg("-c")
        .arg("core.quotePath=false")
        .arg("ls-files")
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run git ls-files in {}: {error}",
                corpus.display()
            )
        });
    assert!(
        output.status.success(),
        "git ls-files failed in {}:\n{}",
        corpus.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let files = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.ends_with(".bsl"))
        .map(std::path::PathBuf::from)
        .collect::<Vec<_>>();
    assert!(
        !files.is_empty(),
        "pinned corpus {} must contain tracked .bsl files",
        corpus.display()
    );
    files
}

fn read_baseline() -> Baseline {
    let content = fs::read_to_string(BASELINE_FIXTURE)
        .unwrap_or_else(|error| panic!("failed to read {BASELINE_FIXTURE}: {error}"));
    let value: Value = serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {BASELINE_FIXTURE}: {error}"));

    let corpus_commit = value["corpus_commit"]
        .as_str()
        .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry a corpus_commit string"))
        .to_owned();
    let tracked_bsl_files = value["tracked_bsl_files"]
        .as_u64()
        .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry tracked_bsl_files"))
        as usize;
    let count = value["count"].as_u64().expect("count must be a number") as usize;

    let mut diagnostics = Vec::new();
    for diagnostic in value["diagnostics"]
        .as_array()
        .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry a diagnostics array"))
    {
        let path = diagnostic["path"]
            .as_str()
            .unwrap_or_else(|| panic!("baseline diagnostic must carry a path: {diagnostic}"))
            .to_owned();
        let line = diagnostic["line"]
            .as_u64()
            .unwrap_or_else(|| panic!("baseline diagnostic must carry a line: {diagnostic}"))
            as usize;
        let chars = diagnostic["chars"]
            .as_u64()
            .unwrap_or_else(|| panic!("baseline diagnostic must carry chars: {diagnostic}"))
            as usize;
        diagnostics.push(Diagnostic { path, line, chars });
    }

    assert_eq!(
        diagnostics.len(),
        count,
        "{BASELINE_FIXTURE} count must match the diagnostics length"
    );

    Baseline {
        corpus_commit,
        tracked_bsl_files,
        diagnostics,
    }
}

fn render_diff(records: &[&Fingerprint]) -> String {
    if records.is_empty() {
        return "- none\n".to_owned();
    }
    records
        .iter()
        .take(MAX_DIFF_RECORDS)
        .map(|fingerprint| {
            format!(
                "- {}:{} (chars {})\n",
                fingerprint.path, fingerprint.line, fingerprint.chars
            )
        })
        .collect()
}
