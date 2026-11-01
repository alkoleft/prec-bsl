//! bsl-language-server SemicolonPresence parity test for
//! `ПроверкаТочекЗапятой` (issue #10, snapshot validation — see the parity
//! scope statement in `spec/parser-strategy.md` "Semicolon presence
//! contract" and the recipe/known-noise notes in
//! `tests/fixtures/semicolon-parity/bsl-ls-semicolon-baseline.json`).
//!
//! Env-gated like `tests/line_length_parity.rs`: it runs only when
//! `PREC_BSL_PARITY_CORPUS` points at a readable yaxunit checkout AND the
//! pinned bsl-ls jar is present. Parity is a snapshot pinned to the baseline
//! `corpus_commit` (d21ce6ff), not to the checkout's current HEAD: both
//! tests clone the corpus into a disposable temp copy (object-sharing git
//! clone, pinned commit checked out there — the `tests/spacing_parity.rs`
//! precedent) and never mutate the checkout referenced by the env var. When
//! the repository no longer contains the pinned commit (upstream moved on
//! and dropped it from local history) the tests skip, not fail — the
//! `tests/line_break_parity.rs` `git archive` precedent.
//!
//! The detector is exercised through the public unit seam
//! `prec_bsl::semicolon_presence::find_missing_semicolons`, not through the
//! pipeline: the fingerprint needs `(relpath, line)` directly (the 1-based
//! stop-token line of each violation), and re-parsing those out of message
//! text would test the message format instead of the parity.
//!
//! Assertions (matching Task 6 of the issue #10 plan):
//! (a) fingerprint parity: the bsl-ls `SemicolonPresence` diagnostics on
//!     the pristine pinned clone (LSP 0-based lines normalized to 1-based)
//!     match the baseline fingerprint, and the detector fingerprint over
//!     the same tracked .bsl files matches it too — 0 missing /
//!     0 unexpected (no residue classes needed on this corpus snapshot);
//! (b) post-fix parity (`fix_missing_semicolons` applied in the disposable
//!     clone): bsl-ls reports 0 `SemicolonPresence` diagnostics outside
//!     (1) files the fix refused by documented policy (unsafe parse errors
//!     / lone `\r`) — their list is pinned in the baseline — and (2)
//!     detector residue classes (none on this snapshot); and no NEW
//!     stable-code diagnostics appear on the fixed files (the insertion is
//!     a single `;` before a trailing comment or end of line). The
//!     `UnusedLocalMethod`/`UnusedLocalVariable` codes are excluded from
//!     the new-diagnostics diff: bsl-ls reruns on an identical corpus
//!     differ in exactly those codes (known noise, documented in both the
//!     spacing and the semicolon baseline fixtures).
//!
//! Baseline refresh instructions (corpus drift): re-run bsl-ls v1.0.7 over
//! a clone of the new corpus commit (recipe in the baseline fixture), pin
//! the new commit in `corpus_commit`, and replace
//! `tests/fixtures/semicolon-parity/bsl-ls-semicolon-baseline.json`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use prec_bsl::semicolon_presence::{find_missing_semicolons, fix_missing_semicolons};
use serde_json::Value;

const PARITY_CORPUS_ENV: &str = "PREC_BSL_PARITY_CORPUS";
const JAR_PATH: &str = "/tmp/opencode/bsl_ls.jar";
const BASELINE_FIXTURE: &str = "tests/fixtures/semicolon-parity/bsl-ls-semicolon-baseline.json";
/// Records printed per diff section before the panic message is truncated.
const MAX_DIFF_RECORDS: usize = 20;
/// bsl-ls diagnostic codes that differ between reruns on an identical
/// corpus (internal tree/caching nondeterminism — known noise documented in
/// the baseline fixtures of both spacing and semicolon parity). Excluded
/// from the post-fix new-diagnostics diff.
const NONDETERMINISTIC_CODES: &[&str] = &["UnusedLocalMethod", "UnusedLocalVariable"];

#[test]
fn semicolon_detector_matches_bsl_ls_baseline_on_yaxunit_corpus() {
    let Some(corpus) = parity_corpus() else {
        eprintln!(
            "skipping bsl-ls semicolon parity: {PARITY_CORPUS_ENV} is not set to a readable \
             yaxunit checkout"
        );
        return;
    };
    if !Path::new(JAR_PATH).is_file() {
        eprintln!("skipping bsl-ls semicolon parity: jar {JAR_PATH} is not present");
        return;
    }
    let baseline = read_baseline();

    // Fingerprint parity never touches the checkout referenced by the env
    // var: the pinned commit is cloned into a disposable temp copy and the
    // detector runs over its tracked .bsl files (the original can be at a
    // later HEAD — the snapshot is pinned to the baseline commit, not to
    // the working tree).
    let Some(clone) = clone_pinned_corpus(&corpus, &baseline.corpus_commit, "fingerprint") else {
        return;
    };
    let tracked_bsl = tracked_bsl_files(&clone);
    assert_eq!(
        tracked_bsl.len(),
        baseline.tracked_bsl_files,
        "pinned corpus commit must contain exactly the tracked .bsl files recorded in the \
         baseline"
    );

    // Detector fingerprint over the pinned clone: (relpath, 1-based
    // stop-token line) per violation.
    let mut detector = BTreeSet::new();
    for relpath in &tracked_bsl {
        let content = read_file(&clone.join(relpath), relpath);
        let violations = find_missing_semicolons(&content).unwrap_or_else(|error| {
            panic!(
                "detector failed to parse corpus file {}: {error:?}",
                relpath.display()
            )
        });
        for violation in violations {
            detector.insert(Fingerprint {
                path: relpath.to_string_lossy().replace('\\', "/"),
                line: violation.line,
            });
        }
    }

    // bsl-ls fingerprint over the same clone, LSP 0-based lines normalized
    // to the detector's 1-based stop-token lines (SKEP-102).
    let outdir = std::env::temp_dir().join(format!(
        "prec-bsl-semicolon-parity-fingerprint-{}",
        std::process::id()
    ));
    let bsl_ls = run_bsl_ls(&clone, &outdir);
    let _ = fs::remove_dir_all(&outdir);
    let mut bsl_ls_fingerprint = BTreeSet::new();
    for (code, path, line, _) in &bsl_ls {
        if code == "SemicolonPresence" {
            bsl_ls_fingerprint.insert(Fingerprint {
                path: path.clone(),
                line: line + 1,
            });
        }
    }

    let expected: BTreeSet<Fingerprint> = baseline
        .diagnostics
        .iter()
        .map(|diagnostic| Fingerprint {
            path: diagnostic.path.clone(),
            line: diagnostic.line,
        })
        .collect();

    // The bsl-ls run over the clone must first reproduce the baseline
    // snapshot itself (guards a stale baseline recorded from a different
    // corpus state than the pinned commit).
    assert_eq!(
        bsl_ls_fingerprint, expected,
        "bsl-ls SemicolonPresence fingerprint on the pinned clone must reproduce the baseline \
         snapshot"
    );

    let missing: Vec<&Fingerprint> = expected.difference(&detector).collect();
    let unexpected: Vec<&Fingerprint> = detector.difference(&expected).collect();

    let report = format!(
        "bsl-ls SemicolonPresence parity: baseline {} diagnostics, bsl-ls rerun {} , detector \
         found {} on {} tracked .bsl files; missing (in baseline, not detected): {}, unexpected \
         (detected, not in baseline): {}",
        expected.len(),
        bsl_ls_fingerprint.len(),
        detector.len(),
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
    println!(
        "parity: {}/{} expected diagnostics matched, 0 missing, 0 unexpected",
        expected.len(),
        expected.len()
    );
}

#[test]
fn semicolon_post_fix_parity_on_yaxunit_corpus() {
    let Some(corpus) = parity_corpus() else {
        eprintln!(
            "skipping bsl-ls semicolon post-fix parity: {PARITY_CORPUS_ENV} is not set to a \
             readable yaxunit checkout"
        );
        return;
    };
    if !Path::new(JAR_PATH).is_file() {
        eprintln!("skipping bsl-ls semicolon post-fix parity: jar {JAR_PATH} is not present");
        return;
    }
    let baseline = read_baseline();

    // The fix is applied inside a disposable temp clone of the pinned
    // commit; the checkout referenced by the env var is never mutated.
    let Some(clone) = clone_pinned_corpus(&corpus, &baseline.corpus_commit, "post-fix") else {
        return;
    };
    let tracked_bsl = tracked_bsl_files(&clone);
    assert_eq!(
        tracked_bsl.len(),
        baseline.tracked_bsl_files,
        "pinned corpus commit must contain exactly the tracked .bsl files recorded in the \
         baseline"
    );

    let outdir = std::env::temp_dir().join(format!(
        "prec-bsl-semicolon-parity-post-{}",
        std::process::id()
    ));

    // Pristine run of the disposable clone: the baseline for the
    // new-diagnostics diff below.
    let pristine = run_bsl_ls(&clone, &outdir.join("pristine"));
    let pristine_semicolon = semicolon_diagnostics(&pristine);
    assert_eq!(
        pristine_semicolon.len(),
        baseline.diagnostics.len(),
        "pristine SemicolonPresence count on the disposable clone must match the baseline \
         snapshot"
    );

    // Apply the fixer to every tracked .bsl file of the clone. Files where
    // violations exist but the fixer refuses to write (unsafe parse errors
    // / lone `\r`, whole-file refusal policy) are collected for the
    // refused-files exemption below; on this corpus snapshot the baseline
    // pins zero such files.
    let mut changed_files: BTreeSet<String> = BTreeSet::new();
    let mut refused_files: BTreeSet<String> = BTreeSet::new();
    for relpath in &tracked_bsl {
        let path = clone.join(relpath);
        let input = read_file(&path, relpath);
        let rel = relpath.to_string_lossy().replace('\\', "/");
        let violations = find_missing_semicolons(&input).unwrap_or_else(|error| {
            panic!(
                "detector failed to parse corpus file {}: {error:?}",
                relpath.display()
            )
        });
        let fixed = fix_missing_semicolons(&input).unwrap_or_else(|error| {
            panic!(
                "fixer failed to parse corpus file {}: {error:?}",
                relpath.display()
            )
        });
        if fixed != input {
            fs::write(&path, fixed).unwrap_or_else(|error| {
                panic!("failed to write corpus file {}: {error}", relpath.display())
            });
            changed_files.insert(rel);
        } else if !violations.is_empty() {
            refused_files.insert(rel);
        }
    }

    // The refused set observed at runtime must match the baseline pin: a
    // file newly refusing (or no longer refusing) is a contract change.
    let refused_pinned: BTreeSet<String> = baseline.fix_refused_files.iter().cloned().collect();
    assert_eq!(
        refused_files, refused_pinned,
        "the set of files the semicolon fix refuses by documented policy must match the \
         baseline snapshot"
    );

    // Post-fix run: SemicolonPresence must be 0 outside refused files and
    // residue classes (both empty on this snapshot — so the assertion is a
    // plain zero here, kept explicit for the drift-refresh workflow).
    let post = run_bsl_ls(&clone, &outdir.join("post"));
    let post_semicolon = semicolon_diagnostics(&post);
    let mut offenders = Vec::new();
    for (path, line) in &post_semicolon {
        if refused_pinned.contains(path) {
            continue;
        }
        offenders.push(format!("- {path}:{}", line + 1));
    }
    assert!(
        offenders.is_empty(),
        "post-fix SemicolonPresence must be 0 outside fix-refused files and residue classes; \
         found {}:\n{}",
        offenders.len(),
        offenders
            .iter()
            .take(MAX_DIFF_RECORDS)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );

    // New non-semicolon diagnostics introduced by the fix: every diagnostic
    // on a changed file outside the nondeterministic codes must already
    // exist in the pristine run of the same clone with the identical
    // (code, path, line, character) position. The insertion adds no lines
    // and only appends `;` at a stop token, so diagnostic positions on the
    // changed lines stay comparable modulo the one-character shift at or
    // after the insertion point — a position-insensitive (code, path, line)
    // match with the character folded to the line is accepted as well, and
    // anything else fails with the file/position printed.
    let mut new_diagnostics = Vec::new();
    for (code, path, line, character) in &post {
        if code == "SemicolonPresence"
            || !changed_files.contains(path)
            || NONDETERMINISTIC_CODES.contains(&code.as_str())
        {
            continue;
        }
        let exact_match = pristine
            .iter()
            .any(|(p_code, p_path, p_line, p_character)| {
                p_code == code && p_path == path && p_line == line && p_character == character
            });
        let line_match = pristine
            .iter()
            .any(|(p_code, p_path, p_line, _)| p_code == code && p_path == path && p_line == line);
        if !exact_match && !line_match {
            new_diagnostics.push(format!("- {path}:{line}:{character} {code}"));
        }
    }
    assert!(
        new_diagnostics.is_empty(),
        "the semicolon fix must not introduce new stable-code diagnostics on changed files; \
         found {} (first {MAX_DIFF_RECORDS}):\n{}",
        new_diagnostics.len(),
        new_diagnostics
            .iter()
            .take(MAX_DIFF_RECORDS)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );

    println!(
        "post-fix parity: fix changed {} of {} tracked .bsl files; SemicolonPresence after \
         fix: {} (0 outside refused/residue policy), new stable-code diagnostics: 0",
        changed_files.len(),
        tracked_bsl.len(),
        post_semicolon.len()
    );

    let _ = fs::remove_dir_all(&outdir);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Fingerprint {
    path: String,
    line: usize,
}

struct Baseline {
    corpus_commit: String,
    tracked_bsl_files: usize,
    diagnostics: Vec<Diagnostic>,
    fix_refused_files: Vec<String>,
}

#[derive(Debug)]
struct Diagnostic {
    path: String,
    line: usize,
}

fn parity_corpus() -> Option<PathBuf> {
    let corpus = std::env::var_os(PARITY_CORPUS_ENV)?;
    let corpus = Path::new(&corpus);
    // Cheap readability probe: the baseline corpus always carries its git
    // metadata and .bsl-language-server.json.
    corpus.join(".git").exists().then(|| corpus.to_path_buf())
}

/// Clones the corpus repository into a disposable temp directory sharing
/// objects with the source (cheap) and checks out the pinned baseline
/// commit there. Only the clone is read/mutated by the tests; the checkout
/// referenced by `PREC_BSL_PARITY_CORPUS` stays untouched (AGENTS.md: the
/// parity corpus is read-only). `None` — skip: git is unavailable, or the
/// repository no longer contains the pinned commit (upstream drift; the
/// baseline must be refreshed per the fixture header).
fn clone_pinned_corpus(source: &Path, pinned_commit: &str, tag: &str) -> Option<PathBuf> {
    let target = std::env::temp_dir().join(format!(
        "prec-bsl-semicolon-parity-{tag}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&target);
    let clone_output = Command::new("git")
        .args([
            "clone",
            "--no-hardlinks",
            "--shared",
            "--quiet",
            &source.display().to_string(),
            &target.display().to_string(),
        ])
        .output();
    let cloned = match clone_output {
        Ok(output) => output.status.success(),
        Err(error) => {
            eprintln!(
                "skipping bsl-ls semicolon parity ({tag}): failed to run git on {}: {error}",
                source.display()
            );
            return None;
        }
    };
    if !cloned {
        eprintln!(
            "skipping bsl-ls semicolon parity ({tag}): failed to clone corpus {} into a \
             disposable copy",
            source.display()
        );
        return None;
    }
    let checkout_output = Command::new("git")
        .current_dir(&target)
        .args(["checkout", "--quiet", pinned_commit])
        .output();
    let checked_out = match checkout_output {
        Ok(output) => output.status.success(),
        Err(error) => {
            eprintln!(
                "skipping bsl-ls semicolon parity ({tag}): git checkout failed in the \
                 disposable copy: {error}"
            );
            let _ = fs::remove_dir_all(&target);
            return None;
        }
    };
    if !checked_out {
        eprintln!(
            "skipping bsl-ls semicolon parity ({tag}): corpus {} does not contain pinned \
             commit {pinned_commit} (upstream moved on); refresh the baseline per the header \
             of {BASELINE_FIXTURE}",
            source.display()
        );
        let _ = fs::remove_dir_all(&target);
        return None;
    }
    Some(target)
}

/// Tracked .bsl files of the corpus as POSIX-relative paths.
/// `core.quotePath=false` keeps Cyrillic paths unescaped.
fn tracked_bsl_files(corpus: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .current_dir(corpus)
        .args(["-c", "core.quotePath=false", "ls-files", "--", "*.bsl"])
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
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    assert!(
        !files.is_empty(),
        "pinned corpus {} must contain tracked .bsl files",
        corpus.display()
    );
    files
}

fn read_file(path: &Path, relpath: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read corpus file {}: {error}", relpath.display()))
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
    let count = value["count"]
        .as_u64()
        .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry count")) as usize;

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
        diagnostics.push(Diagnostic { path, line });
    }

    assert_eq!(
        diagnostics.len(),
        count,
        "{BASELINE_FIXTURE} count must match the diagnostics length"
    );

    let fix_refused_files = value["fix_refused_files"]
        .as_array()
        .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry a fix_refused_files array"))
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .unwrap_or_else(|| panic!("fix_refused_files entries must be strings: {entry}"))
                .to_owned()
        })
        .collect();

    Baseline {
        corpus_commit,
        tracked_bsl_files,
        diagnostics,
        fix_refused_files,
    }
}

fn render_diff(records: &[&Fingerprint]) -> String {
    if records.is_empty() {
        return "- none\n".to_owned();
    }
    records
        .iter()
        .take(MAX_DIFF_RECORDS)
        .map(|fingerprint| format!("- {}:{}\n", fingerprint.path, fingerprint.line))
        .collect()
}

/// Runs bsl-ls analyze over the corpus and returns ALL diagnostics as
/// `(code, relpath, 0-based line, start character)` tuples with
/// percent-decoded POSIX paths re-anchored onto the corpus (the
/// `tests/spacing_parity.rs` URI anchoring precedent).
fn run_bsl_ls(corpus: &Path, outdir: &Path) -> Vec<(String, String, usize, usize)> {
    fs::create_dir_all(outdir).unwrap_or_else(|error| {
        panic!(
            "failed to create bsl-ls output dir {}: {error}",
            outdir.display()
        )
    });
    let output = Command::new("java")
        .arg("-jar")
        .arg(JAR_PATH)
        .arg("analyze")
        .arg("-s")
        .arg(corpus)
        .arg("-r")
        .arg("json")
        .arg("-o")
        .arg(outdir)
        .arg("-q")
        .output()
        .unwrap_or_else(|error| panic!("failed to run bsl-ls on {}: {error}", corpus.display()));
    assert!(
        output.status.success(),
        "bsl-ls analyze failed on {}:\n{}",
        corpus.display(),
        String::from_utf8_lossy(&output.stderr)
    );

    let report = outdir.join("bsl-json.json");
    let content = fs::read_to_string(&report).unwrap_or_else(|error| {
        panic!("failed to read bsl-ls report {}: {error}", report.display())
    });
    let value: Value = serde_json::from_str(&content).unwrap_or_else(|error| {
        panic!(
            "failed to parse bsl-ls report {}: {error}",
            report.display()
        )
    });

    // bsl-ls emits source URIs as `file://<cwd-relative>/…` paths with `..`
    // segments when the corpus is outside the process CWD; canonicalizing
    // the corpus dir once and comparing canonical prefixes would not match
    // those. Instead the URI is resolved lexically against the corpus
    // directory and canonicalized together with the corpus path for every
    // diagnostic.
    let corpus_canonical = corpus.canonicalize().unwrap_or_else(|error| {
        panic!(
            "failed to canonicalize corpus {}: {error}",
            corpus.display()
        )
    });
    let corpus_prefix = format!("file://{}/", corpus.display());
    let mut diagnostics = Vec::new();
    // Path re-anchoring failures must be loud: a silently dropped fileinfo
    // would deflate every count and the parity snapshot would validate
    // against incomplete data.
    let mut dropped_fileinfos = 0usize;
    let mut dropped_example: Option<String> = None;
    for fileinfo in value["fileinfos"].as_array().cloned().unwrap_or_default() {
        let Some(path) = fileinfo["path"].as_str() else {
            continue;
        };
        let relative = path
            .strip_prefix(&corpus_prefix)
            .map(percent_decode)
            .unwrap_or_else(|| {
                // Path outside the corpus prefix (e.g. `file://../../…`):
                // resolve lexically against the CWD, then re-anchor on the
                // corpus.
                let decoded = percent_decode(path.trim_start_matches("file://"));
                let cwd_path = Path::new(&decoded);
                cwd_path
                    .canonicalize()
                    .ok()
                    .and_then(|absolute| {
                        absolute
                            .strip_prefix(&corpus_canonical)
                            .ok()
                            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                    })
                    .unwrap_or_default()
            });
        if relative.is_empty() {
            dropped_fileinfos += 1;
            dropped_example.get_or_insert_with(|| path.to_owned());
            continue;
        }
        for diagnostic in fileinfo["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let Some(code) = diagnostic["code"].as_str() else {
                continue;
            };
            let range = &diagnostic["range"];
            let start = &range["start"];
            diagnostics.push((
                code.to_owned(),
                relative.clone(),
                start["line"].as_u64().unwrap_or_default() as usize,
                start["character"].as_u64().unwrap_or_default() as usize,
            ));
        }
    }
    if dropped_fileinfos > 0 {
        panic!(
            "bsl-ls report for {} contains {} fileinfo path(s) that could not be re-anchored \
             onto the corpus (e.g. {}); diagnostics in them would be silently dropped — \
             adjust the URI anchoring in this test",
            corpus.display(),
            dropped_fileinfos,
            dropped_example.as_deref().unwrap_or("<unknown>")
        );
    }
    diagnostics
}

/// SemicolonPresence subset with LSP 0-based lines kept for the pristine vs
/// post comparison (the two bsl-ls runs are compared to each other, not to
/// the 1-based baseline).
fn semicolon_diagnostics(diagnostics: &[(String, String, usize, usize)]) -> Vec<(String, usize)> {
    diagnostics
        .iter()
        .filter(|(code, _, _, _)| code == "SemicolonPresence")
        .map(|(_, path, line, _)| (path.clone(), *line))
        .collect()
}

/// Minimal percent-decoder for `file:///…` URIs (Cyrillic paths come back
/// percent-encoded; `+` stays `+` because form-encoding is not in play).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &input[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                output.push(byte);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}
