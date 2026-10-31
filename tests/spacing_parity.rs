//! bsl-language-server spacing parity test for `ПробелыВокругОператоров` and
//! `ЛишниеЗапятыеВКонце` (issue #9, snapshot validation — see the residue
//! policy in `tests/fixtures/spacing-parity/bsl-ls-spacing-baseline.json`).
//!
//! Env-gated like `tests/line_length_parity.rs`: it runs only when
//! `PREC_BSL_PARITY_CORPUS` points at a readable yaxunit clone AND the pinned
//! bsl-ls jar is present; it skips (not fails) when the checkout's
//! `git rev-parse HEAD` differs from the baseline's pinned `corpus_commit` —
//! parity is validated as a snapshot, not as a continuous guarantee against
//! upstream drift.
//!
//! The corpus pointed at by the env var is MUTATED in place (the two fixers
//! are applied to every tracked .bsl file and re-run for idempotence), so the
//! env must reference a disposable clone, never the original checkout:
//! `git clone /path/to/yaxunit /tmp/parity-yaxunit` and set the env to the
//! clone.
//!
//! Assertions (matching Task 3 of the issue #9 plan):
//! (a) bsl-ls analyze on the pristine corpus, filtered to
//!     `diagnostics[].code ∈ {MissingSpace, ExtraCommas}`, matches the
//!     baseline pre-fix counts;
//! (b) `fix_operator_spacing` then `fix_extra_trailing_commas` applied to
//!     every tracked .bsl file of the corpus;
//! (c) re-run of bsl-ls: `ExtraCommas == 0`, `MissingSpace` matches the
//!     baseline post-fix count, and every post-fix `MissingSpace` diagnostic
//!     is classifiable mechanically (by the token at the diagnostic range on
//!     the post-fix text) into one of the baseline's residue classes:
//!     unary `-`/`+` after an operator byte, `(`, `[`, `,`, the word
//!     `Возврат`/`Return` (case-insensitive) or file start, `,`/`;`, or a
//!     keyword word-token — with per-class counts matching the baseline
//!     residue counts (regression guard against the fixers fixing more or
//!     less);
//! (d) a second pass of both fixers over the fixed corpus changes nothing
//!     (corpus-level idempotence).
//!
//! Baseline refresh instructions (corpus drift): re-run bsl-ls v1.0.7 over a
//! fresh clone of the new corpus commit, pin the new commit in
//! `corpus_commit`, and replace the pre-fix/post-fix counts and residue
//! counts in
//! `tests/fixtures/spacing-parity/bsl-ls-spacing-baseline.json`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use prec_bsl::text_fixers::{fix_extra_trailing_commas, fix_operator_spacing};
use serde_json::Value;

const PARITY_CORPUS_ENV: &str = "PREC_BSL_PARITY_CORPUS";
const JAR_PATH: &str = "/tmp/opencode/bsl_ls.jar";
const BASELINE_FIXTURE: &str = "tests/fixtures/spacing-parity/bsl-ls-spacing-baseline.json";
/// Records printed per failing residue class before the panic message is
/// truncated.
const MAX_RESIDUE_RECORDS: usize = 20;

#[test]
fn spacing_fixers_match_bsl_ls_residue_policy_on_yaxunit_corpus() {
    let Some(corpus) = parity_corpus() else {
        eprintln!(
            "skipping bsl-ls spacing parity: {PARITY_CORPUS_ENV} is not set to a readable \
             yaxunit clone"
        );
        return;
    };
    if !Path::new(JAR_PATH).is_file() {
        eprintln!("skipping bsl-ls spacing parity: jar {JAR_PATH} is not present");
        return;
    }

    let baseline = read_baseline();
    let corpus_head = git_head(&corpus);
    if corpus_head != baseline.corpus_commit {
        eprintln!(
            "skipping bsl-ls spacing parity: corpus {} is at {corpus_head}, baseline is \
             pinned to {}; re-run bsl-ls (recipe in the baseline fixture) and refresh \
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
    let corpus_is_pristine = git_worktree_is_clean(&corpus);
    assert!(
        corpus_is_pristine,
        "parity corpus {} must start pristine (HEAD == {corpus_head}); point \
         {PARITY_CORPUS_ENV} at a fresh disposable clone",
        corpus.display()
    );

    // (a) pre-fix bsl-ls run, filtered to the two codes, must match baseline.
    let outdir =
        std::env::temp_dir().join(format!("prec-bsl-spacing-parity-{}", std::process::id()));
    let pre = run_bsl_ls(&corpus, &outdir.join("pre"));
    let pre_counts = counts(&pre);
    assert_eq!(
        pre_counts.missing_space, baseline.pre_missing_space,
        "pre-fix MissingSpace count must match the baseline snapshot"
    );
    assert_eq!(
        pre_counts.extra_commas, baseline.pre_extra_commas,
        "pre-fix ExtraCommas count must match the baseline snapshot"
    );

    // (b) apply the two fixers to every tracked .bsl file, in the fixed
    // order operator -> commas.
    let mut changed_files = 0usize;
    for relpath in &tracked_bsl {
        let path = corpus.join(relpath);
        let input = read_file(&path, relpath);
        let fixed = fix_extra_trailing_commas(&fix_operator_spacing(&input));
        if fixed != input {
            fs::write(&path, fixed).unwrap_or_else(|error| {
                panic!("failed to write corpus file {}: {error}", relpath.display())
            });
            changed_files += 1;
        }
    }

    // (c) post-fix bsl-ls run: ExtraCommas must be zero, MissingSpace must
    // fall into the residue classes.
    let post = run_bsl_ls(&corpus, &outdir.join("post"));
    let post_counts = counts(&post);
    assert_eq!(
        post_counts.extra_commas, 0,
        "post-fix ExtraCommas must be 0: fix_extra_trailing_commas must remove every \
         bsl-ls-flagged trailing comma in the corpus"
    );
    assert_eq!(
        post_counts.missing_space, baseline.post_missing_space,
        "post-fix MissingSpace count must match the baseline snapshot (regression guard: the \
         fixers must fix exactly as much as the pinned snapshot records)"
    );

    let residue = classify_residue(&post, &corpus, &tracked_bsl);
    assert_eq!(
        residue.comma_semicolon, baseline.residue_comma_semicolon,
        "post-fix comma/semicolon residue count must match the baseline snapshot"
    );
    assert_eq!(
        residue.unary_sign, baseline.residue_unary_sign,
        "post-fix unary-sign residue count must match the baseline snapshot"
    );
    assert_eq!(
        residue.keyword, baseline.residue_keyword,
        "post-fix keyword residue count must match the baseline snapshot"
    );
    assert_eq!(
        residue.comma_semicolon + residue.unary_sign + residue.keyword,
        post_counts.missing_space,
        "every post-fix MissingSpace diagnostic must fall into one of the residue classes"
    );
    let report = format!(
        "bsl-ls spacing parity: corpus {corpus_head}, pre {} MissingSpace / {} ExtraCommas, \
         post {} MissingSpace / 0 ExtraCommas, fixers changed {changed_files} of {} tracked \
         .bsl files; residue classes: unary {} , comma/semicolon {} , keyword {}",
        pre_counts.missing_space,
        pre_counts.extra_commas,
        post_counts.missing_space,
        tracked_bsl.len(),
        residue.unary_sign,
        residue.comma_semicolon,
        residue.keyword,
    );
    println!("{report}");

    if !residue.unclassified.is_empty() {
        panic!(
            "{report}\n\npost-fix MissingSpace diagnostics outside the residue policy (first \
             {MAX_RESIDUE_RECORDS}):\n{}",
            render_unclassified(&residue.unclassified)
        );
    }

    // (d) corpus-level idempotence: a second pass of both fixers must be a
    // no-op on the already-fixed corpus.
    let mut second_pass_changes = 0usize;
    for relpath in &tracked_bsl {
        let path = corpus.join(relpath);
        let input = read_file(&path, relpath);
        let fixed = fix_extra_trailing_commas(&fix_operator_spacing(&input));
        if fixed != input {
            second_pass_changes += 1;
        }
    }
    assert_eq!(
        second_pass_changes, 0,
        "second pass of the spacing fixers must not change any corpus file"
    );
    println!(
        "parity: idempotent second pass over {} tracked .bsl files (0 changes)",
        tracked_bsl.len()
    );

    // Leave the disposable clone pristine so consecutive test runs (and the
    // pristine-corpus assertion above) keep working without a manual reset.
    restore_corpus(&corpus);
    let _ = fs::remove_dir_all(&outdir);
}

/// Restores the disposable corpus clone to its pinned HEAD. Only tracked
/// modifications made by this test are reverted; failure is not fatal (the
/// clone is disposable by contract).
fn restore_corpus(corpus: &Path) {
    let restored = Command::new("git")
        .current_dir(corpus)
        .args(["checkout", "--", "."])
        .output()
        .is_ok_and(|output| output.status.success());
    if !restored {
        eprintln!(
            "warning: failed to restore corpus clone {} to HEAD; reset it before the next run",
            corpus.display()
        );
    }
}

struct Counts {
    missing_space: usize,
    extra_commas: usize,
}

struct Residue {
    unary_sign: usize,
    comma_semicolon: usize,
    keyword: usize,
    unclassified: Vec<String>,
}

#[derive(Debug)]
struct Diagnostic {
    path: String,
    line: usize,
    /// LSP range start, in UTF-16 code units.
    start_character: usize,
    /// LSP range end, in UTF-16 code units.
    end_character: usize,
}

fn parity_corpus() -> Option<PathBuf> {
    let corpus = std::env::var_os(PARITY_CORPUS_ENV)?;
    let corpus = Path::new(&corpus);
    // Cheap readability probe: the baseline corpus always carries its git
    // metadata.
    corpus.join(".git").exists().then(|| corpus.to_path_buf())
}

fn git_head(corpus: &Path) -> String {
    let output = git(corpus, ["rev-parse", "HEAD"]);
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn git_worktree_is_clean(corpus: &Path) -> bool {
    let output = git(corpus, ["status", "--porcelain"]);
    String::from_utf8_lossy(&output.stdout).trim().is_empty()
}

fn git(corpus: &Path, args: [&str; 2]) -> std::process::Output {
    let output = Command::new("git")
        .current_dir(corpus)
        .args(args)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run git {} in {}: {error}",
                args[0],
                corpus.display()
            )
        });
    assert!(
        output.status.success(),
        "git {} failed in {}:\n{}",
        args[0],
        corpus.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
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

/// Runs bsl-ls analyze over the corpus and returns the filtered diagnostics
/// (`MissingSpace` + `ExtraCommas`) with percent-decoded POSIX paths.
fn run_bsl_ls(corpus: &Path, outdir: &Path) -> Vec<(String, Diagnostic)> {
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
    // segments when the corpus is outside the process CWD; canonicalizing the
    // corpus dir once and comparing canonical prefixes would not match those.
    // Instead the URI is resolved lexically against the corpus directory and
    // canonicalized together with the corpus path for every diagnostic.
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
            let code = diagnostic["code"].as_str().unwrap_or_default().to_owned();
            let code = code.as_str();
            if code != "MissingSpace" && code != "ExtraCommas" {
                continue;
            }
            let range = diagnostic["range"].clone();
            let start = &range["start"];
            let end = &range["end"];
            diagnostics.push((
                code.to_owned(),
                Diagnostic {
                    path: relative.clone(),
                    line: start["line"].as_u64().unwrap_or_default() as usize,
                    start_character: start["character"].as_u64().unwrap_or_default() as usize,
                    end_character: end["character"].as_u64().unwrap_or_default() as usize,
                },
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

fn counts(diagnostics: &[(String, Diagnostic)]) -> Counts {
    let mut counts = Counts {
        missing_space: 0,
        extra_commas: 0,
    };
    for (code, _) in diagnostics {
        match code.as_str() {
            "MissingSpace" => counts.missing_space += 1,
            "ExtraCommas" => counts.extra_commas += 1,
            _ => {}
        }
    }
    counts
}

/// Classifies post-fix MissingSpace diagnostics mechanically by the token at
/// the diagnostic range on the post-fix text (never by message text):
/// unary `-`/`+` after an operator byte, `(`, `[`, `,`, the word
/// `Возврат`/`Return` (case-insensitive) or file start; `,`/`;`; keyword
/// word-tokens.
fn classify_residue(
    diagnostics: &[(String, Diagnostic)],
    corpus: &Path,
    tracked_bsl: &[PathBuf],
) -> Residue {
    let mut residue = Residue {
        unary_sign: 0,
        comma_semicolon: 0,
        keyword: 0,
        unclassified: Vec::new(),
    };

    for (code, diagnostic) in diagnostics {
        if code != "MissingSpace" {
            continue;
        }
        let relpath = tracked_bsl
            .iter()
            .find(|candidate| candidate.to_string_lossy() == diagnostic.path)
            .cloned()
            .unwrap_or_else(|| PathBuf::from(&diagnostic.path));
        let path = corpus.join(&relpath);
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) => {
                residue.unclassified.push(format!(
                    "- {}:{}:{} (failed to read post-fix file: {error})",
                    diagnostic.path, diagnostic.line, diagnostic.start_character
                ));
                continue;
            }
        };
        let line = content
            .split('\n')
            .nth(diagnostic.line)
            .unwrap_or_default()
            .trim_end_matches('\r');
        let token = utf16_slice(line, diagnostic.start_character, diagnostic.end_character);

        match classify_token(&token) {
            TokenClass::CommaSemicolon => residue.comma_semicolon += 1,
            TokenClass::Keyword => residue.keyword += 1,
            TokenClass::UnarySign => {
                // Walk back from the token over whitespace to the nearest
                // previous character in the whole file.
                match previous_non_whitespace_char(
                    &content,
                    diagnostic.line,
                    diagnostic.start_character,
                ) {
                    // Unary context (bsl-ls UNARY set): an operator byte,
                    // `(`, `[`, `,`, the closing character of the word
                    // `Возврат`/`Return` (case-insensitive), or file start.
                    Some('[' | '(' | ',' | '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>') => {
                        residue.unary_sign += 1
                    }
                    Some(previous)
                        if previous.is_alphanumeric()
                            && previous_word_at(
                                &content,
                                diagnostic.line,
                                diagnostic.start_character,
                            )
                            .is_some_and(|word| {
                                word.eq_ignore_ascii_case("Return")
                                    || word.to_lowercase() == "возврат"
                            }) =>
                    {
                        residue.unary_sign += 1
                    }
                    None => residue.unary_sign += 1,
                    Some(previous) => residue.unclassified.push(format!(
                        "- {}:{}:{} token '{}' after non-unary predecessor '{}'",
                        diagnostic.path,
                        diagnostic.line,
                        diagnostic.start_character,
                        token,
                        previous
                    )),
                }
            }
            TokenClass::Other => residue.unclassified.push(format!(
                "- {}:{}:{} token '{}'",
                diagnostic.path, diagnostic.line, diagnostic.start_character, token
            )),
        }
    }

    residue
}

enum TokenClass {
    UnarySign,
    CommaSemicolon,
    Keyword,
    Other,
}

fn classify_token(token: &str) -> TokenClass {
    if token == "-" || token == "+" {
        TokenClass::UnarySign
    } else if token == "," || token == ";" {
        TokenClass::CommaSemicolon
    } else if !token.is_empty()
        && token
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
    {
        TokenClass::Keyword
    } else {
        TokenClass::Other
    }
}

/// Extracts the substring of `line` between two UTF-16 code-unit offsets
/// (LSP `character` positions are UTF-16 based).
fn utf16_slice(line: &str, start_units: usize, end_units: usize) -> String {
    let mut units = 0usize;
    let mut start_char = None;
    let mut end_char = line.len();
    for (byte_index, character) in line.char_indices() {
        if start_char.is_none() && units >= start_units {
            start_char = Some(byte_index);
        }
        if units >= end_units {
            end_char = byte_index;
            break;
        }
        units += character.len_utf16();
    }
    line[start_char.unwrap_or(line.len())..end_char.max(start_char.unwrap_or(line.len()))]
        .to_owned()
}

/// Nearest character before the UTF-16 offset `units` on `line_no`, skipping
/// whitespace (spaces, tabs and line breaks across earlier lines).
fn previous_non_whitespace_char(content: &str, line_no: usize, units: usize) -> Option<char> {
    let lines: Vec<&str> = content.split('\n').collect();
    let offset = utf16_slice(lines[line_no.clamp(0, lines.len() - 1)], 0, units);
    for character in offset.chars().rev() {
        if !character.is_whitespace() {
            return Some(character);
        }
    }
    for line in lines[..line_no.clamp(0, lines.len())].iter().rev() {
        for character in line.trim_end_matches('\r').chars().rev() {
            if !character.is_whitespace() {
                return Some(character);
            }
        }
    }
    None
}

/// The identifier word ending at UTF-16 offset `units` on `line_no`, if the
/// nearest preceding non-whitespace run is a word (for the `Возврат`/`Return`
/// unary-context check). The word is walked back from the diagnostic position
/// within its own line only: bsl-ls keywords never span lines.
fn previous_word_at(content: &str, line_no: usize, units: usize) -> Option<String> {
    let lines: Vec<&str> = content.split('\n').collect();
    let line = lines.get(line_no.clamp(0, lines.len() - 1))?;
    let offset = utf16_slice(line, 0, units);
    let mut word = String::new();
    for character in offset.chars().rev() {
        if character.is_whitespace() {
            break;
        }
        if !(character.is_alphanumeric() || character == '_') {
            return None;
        }
        word.insert(0, character);
    }
    if word.is_empty() { None } else { Some(word) }
}

struct Baseline {
    corpus_commit: String,
    tracked_bsl_files: usize,
    pre_missing_space: usize,
    pre_extra_commas: usize,
    post_missing_space: usize,
    residue_comma_semicolon: usize,
    residue_unary_sign: usize,
    residue_keyword: usize,
}

fn read_baseline() -> Baseline {
    let content = fs::read_to_string(BASELINE_FIXTURE)
        .unwrap_or_else(|error| panic!("failed to read {BASELINE_FIXTURE}: {error}"));
    let value: Value = serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {BASELINE_FIXTURE}: {error}"));

    Baseline {
        corpus_commit: value["corpus_commit"]
            .as_str()
            .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry a corpus_commit string"))
            .to_owned(),
        tracked_bsl_files: value["tracked_bsl_files"]
            .as_u64()
            .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry tracked_bsl_files"))
            as usize,
        pre_missing_space: value["pre_fix_counts"]["MissingSpace"]
            .as_u64()
            .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry pre_fix_counts.MissingSpace"))
            as usize,
        pre_extra_commas: value["pre_fix_counts"]["ExtraCommas"]
            .as_u64()
            .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry pre_fix_counts.ExtraCommas"))
            as usize,
        post_missing_space: value["post_fix_counts"]["MissingSpace"]
            .as_u64()
            .unwrap_or_else(|| panic!("{BASELINE_FIXTURE} must carry post_fix_counts.MissingSpace"))
            as usize,
        residue_comma_semicolon:
            value["residue_policy"]["post_fix_residue_counts"]["comma_semicolon"]
                .as_u64()
                .unwrap_or_else(|| {
                    panic!(
                        "{BASELINE_FIXTURE} must carry \
                     residue_policy.post_fix_residue_counts.comma_semicolon"
                    )
                }) as usize,
        residue_unary_sign: value["residue_policy"]["post_fix_residue_counts"]["unary_sign"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!(
                    "{BASELINE_FIXTURE} must carry \
                     residue_policy.post_fix_residue_counts.unary_sign"
                )
            }) as usize,
        residue_keyword: value["residue_policy"]["post_fix_residue_counts"]["keyword"]
            .as_u64()
            .unwrap_or_else(|| {
                panic!(
                    "{BASELINE_FIXTURE} must carry residue_policy.post_fix_residue_counts.keyword"
                )
            }) as usize,
    }
}

fn render_unclassified(records: &[String]) -> String {
    if records.is_empty() {
        return "- none\n".to_owned();
    }
    records
        .iter()
        .take(MAX_RESIDUE_RECORDS)
        .map(|record| format!("{record}\n"))
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
