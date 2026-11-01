use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{
    CheckArgs, ExecRulesArgs, FileSelectionMode, FixArgs, MergeCommitSkipMode, PrekHookArgs,
};
use crate::config::{ConfigResolveRequest, resolve_config};
use crate::git_index::{
    collect_changed_files, collect_staged_files, collect_worktree_files, restage_paths,
};
use crate::output::{OutputFormat, render_dry_run_diffs, render_report};
use crate::reference_registry;
use crate::scenario_pipeline::{
    PipelineMode, PipelineReport, PipelineRequest, ScenarioResult, run_pipeline,
};
use crate::source_files::{
    classify_staged_files, collect_source_files, discover_source_roots_from_staged_files,
    discover_source_roots_from_v8project_manifest, parse_source_dir_list, resolve_source_roots,
};

pub fn run_prek_hook(args: &PrekHookArgs) -> i32 {
    let repo_root = match current_repo_root() {
        Ok(repo_root) => repo_root,
        Err(message) => return print_error(message),
    };

    let should_skip_merge = match args.merge_commit_skip {
        MergeCommitSkipMode::None => false,
        MergeCommitSkipMode::Any => match merge_commit_in_progress(&repo_root) {
            Ok(active) => active,
            Err(message) => return print_error(message),
        },
        MergeCommitSkipMode::Remote => match remote_merge_commit_in_progress(&repo_root) {
            Ok(active) => active,
            Err(message) => return print_error(message),
        },
    };
    if should_skip_merge {
        return skip_merge_commit(args.format, args.merge_commit_skip);
    }

    let staged_files = match collect_staged_files(&repo_root) {
        Ok(staged_files) => staged_files,
        Err(error) => return print_error(error.to_string()),
    };
    let Some(config) = resolve_config_for_run(
        &repo_root,
        args.config.clone(),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        None,
    ) else {
        return 1;
    };
    let source_resolution = if let Some(source_dir) = &args.source_dir {
        resolve_source_roots(&repo_root, std::slice::from_ref(source_dir))
    } else {
        discover_source_roots_from_staged_files(&repo_root, &staged_files)
    };
    let Some(roots) = resolve_roots_or_report(source_resolution, PipelineMode::Hook, args.format)
    else {
        return 1;
    };

    let files = classify_staged_files(&roots, &staged_files);
    let report = run_pipeline(
        &reference_registry(),
        PipelineRequest {
            repo_root: &repo_root,
            source_roots: &roots,
            config: &config,
            files,
            mode: PipelineMode::Hook,
        },
    );

    if let Err(error) = restage_paths(&repo_root, &report.modified_paths()) {
        return print_error(error.to_string());
    }

    print!("{}", render_report(&report, args.format));
    report.exit_code()
}

pub fn run_exec_rules(args: &ExecRulesArgs) -> i32 {
    eprintln!("предупреждение: exec-rules устарела, используйте prec-bsl fix --mode all");
    let repo_root = match canonical_repo_root(&args.repo) {
        Ok(repo_root) => repo_root,
        Err(message) => return print_error(message),
    };
    let explicit_source_dirs = args
        .source_dirs
        .as_ref()
        .map(|source_dirs| parse_source_dir_list(&source_dirs.0));
    let Some((config, roots)) = resolve_config_and_roots(
        &repo_root,
        args.config.clone(),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        None,
        explicit_source_dirs.as_deref(),
        PipelineMode::ExecRules,
        args.format,
    ) else {
        return 1;
    };

    let files = match collect_source_files(&roots) {
        Ok(files) => files,
        Err(error) => return print_error(error.to_string()),
    };
    let report = run_pipeline(
        &reference_registry(),
        PipelineRequest {
            repo_root: &repo_root,
            source_roots: &roots,
            config: &config,
            files,
            mode: PipelineMode::ExecRules,
        },
    );

    print!("{}", render_report(&report, args.format));
    report.exit_code()
}

/// `prec-bsl check`: diagnostics only, never writes files.
pub fn run_check(args: &CheckArgs) -> i32 {
    run_check_or_fix(
        &args.repo,
        CheckFixRun::Check,
        args.config.clone(),
        args.source_dirs.as_ref().map(|dirs| dirs.0.clone()),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        args.exclude_rules.as_ref().map(|rules| rules.0.clone()),
        args.format,
        args.mode,
    )
}

/// `prec-bsl fix`: applies fixers, or previews them with `--dry-run`.
/// With `--restage`, applied fixes are added back to the Git index.
pub fn run_fix(args: &FixArgs) -> i32 {
    run_check_or_fix(
        &args.repo,
        CheckFixRun::Fix {
            dry_run: args.dry_run,
            restage: args.restage,
        },
        args.config.clone(),
        args.source_dirs.as_ref().map(|dirs| dirs.0.clone()),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        args.exclude_rules.as_ref().map(|rules| rules.0.clone()),
        args.format,
        args.mode,
    )
}

/// Kind of `check`/`fix` run. `Fix` carries the two fix-only flags: preview
/// without writing (`dry_run`) and restaging fixed files into the Git index
/// (`restage`, ignored in `dry_run`).
enum CheckFixRun {
    Check,
    Fix { dry_run: bool, restage: bool },
}

// 8 positional args: scenario flags stay flat until the 3rd one arrives
// (issue #14 plan: bundling resolve-wiring params is deferred by design).
#[allow(clippy::too_many_arguments)]
fn run_check_or_fix(
    repo: &Path,
    run: CheckFixRun,
    config_path: Option<PathBuf>,
    source_dirs_value: Option<String>,
    rules_value: Option<String>,
    rule_exclusions: Option<String>,
    format: OutputFormat,
    mode: FileSelectionMode,
) -> i32 {
    let repo_root = match canonical_repo_root(repo) {
        Ok(repo_root) => repo_root,
        Err(message) => return print_execution_error(message),
    };
    let explicit_source_dirs = source_dirs_value.as_deref().map(parse_source_dir_list);
    let pipeline_mode = match run {
        CheckFixRun::Check | CheckFixRun::Fix { dry_run: true, .. } => PipelineMode::Check,
        CheckFixRun::Fix { dry_run: false, .. } => PipelineMode::Fix,
    };
    let Some((config, roots)) = resolve_config_and_roots(
        &repo_root,
        config_path,
        rules_value,
        rule_exclusions,
        explicit_source_dirs.as_deref(),
        pipeline_mode,
        format,
    ) else {
        // Configuration and source-root failures are execution errors for
        // check/fix: contract exit code is 2, not the "violations found" 1.
        return 2;
    };

    let files = match select_mode_files(&repo_root, mode, &roots) {
        Ok(files) => files,
        Err(error) => return print_execution_error(error),
    };
    let report = run_pipeline(
        &reference_registry(),
        PipelineRequest {
            repo_root: &repo_root,
            source_roots: &roots,
            config: &config,
            files,
            mode: pipeline_mode,
        },
    );

    // Dry-run kinds: `check` itself and `fix --dry-run`.
    let (dry_run, restage) = match run {
        CheckFixRun::Check => (true, false),
        CheckFixRun::Fix { dry_run, restage } => (dry_run, restage),
    };
    if restage && dry_run {
        // `--restage` has no effect without writing: warn, but keep the run
        // valid so scripts that always pass the flag do not break.
        eprintln!(
            "предупреждение: --restage не действует вместе с --dry-run: изменения не записаны"
        );
    }
    print!("{}", render_report(&report, format));
    if dry_run {
        // The per-file line diff is a text-only preview: appending it after
        // a JSON report would make stdout unparseable, and JSON already
        // carries `proposed_content` per result.
        if format == OutputFormat::Text {
            let diffs = render_dry_run_diffs(&report, &repo_root);
            if !diffs.is_empty() {
                println!("Предлагаемые изменения (не записаны):");
                print!("{diffs}");
            }
        }
    } else if restage {
        // Real fix run only: put the fixed working-tree files back into the
        // Git index, the same restaging step `prek-hook` performs.
        if let Err(error) = restage_paths(&repo_root, &report.modified_paths()) {
            return print_execution_error(error.to_string());
        }
    }
    report.exit_code()
}

/// Selects the file set for `--mode all|changed|index|worktree` and filters it
/// through the source roots and path classification.
fn select_mode_files(
    repo_root: &Path,
    mode: FileSelectionMode,
    roots: &[crate::source_files::SourceRoot],
) -> Result<Vec<crate::source_files::SourceFile>, String> {
    match mode {
        FileSelectionMode::All => collect_source_files(roots).map_err(|error| error.to_string()),
        FileSelectionMode::Index => {
            let staged = collect_staged_files(repo_root).map_err(|error| error.to_string())?;
            Ok(classify_staged_files(roots, &staged))
        }
        FileSelectionMode::Changed => {
            let changed = collect_changed_files(repo_root).map_err(|error| error.to_string())?;
            Ok(classify_staged_files(roots, &changed))
        }
        FileSelectionMode::Worktree => {
            let worktree = collect_worktree_files(repo_root).map_err(|error| error.to_string())?;
            Ok(classify_staged_files(roots, &worktree))
        }
    }
}

fn resolve_config_and_roots(
    repo_root: &Path,
    config_path: Option<PathBuf>,
    rule_override: Option<String>,
    rule_exclusions: Option<String>,
    source_dirs: Option<&[PathBuf]>,
    mode: PipelineMode,
    format: OutputFormat,
) -> Option<(
    crate::config::ResolvedConfig,
    Vec<crate::source_files::SourceRoot>,
)> {
    let config = resolve_config_for_run(repo_root, config_path, rule_override, rule_exclusions)?;

    let resolution = match source_dirs {
        Some(source_dirs) => resolve_source_roots(repo_root, source_dirs),
        None => discover_source_roots_from_v8project_manifest(repo_root)
            .unwrap_or_else(|| resolve_source_roots(repo_root, &[])),
    };
    let roots = resolve_roots_or_report(resolution, mode, format)?;

    Some((config, roots))
}

fn resolve_config_for_run(
    repo_root: &Path,
    config_path: Option<PathBuf>,
    rule_override: Option<String>,
    rule_exclusions: Option<String>,
) -> Option<crate::config::ResolvedConfig> {
    let mut config_request = ConfigResolveRequest::new(repo_root);
    config_request.config_path = config_path;
    config_request.rule_override = rule_override;
    config_request.rule_exclusions = rule_exclusions;
    match resolve_config(&config_request) {
        Ok(config) => Some(config),
        Err(error) => {
            eprintln!("ошибка: {error}");
            None
        }
    }
}

fn resolve_roots_or_report(
    resolution: crate::source_files::SourceRootResolution,
    mode: PipelineMode,
    format: OutputFormat,
) -> Option<Vec<crate::source_files::SourceRoot>> {
    if resolution.diagnostics.is_empty() {
        return Some(resolution.roots);
    }

    let report = PipelineReport {
        mode,
        processed_files: Vec::new(),
        results: resolution
            .diagnostics
            .into_iter()
            .map(|diagnostic| {
                ScenarioResult::hard_failure("source-root", PathBuf::new(), diagnostic.message)
            })
            .collect(),
    };
    print!("{}", render_report(&report, format));
    None
}

fn current_repo_root() -> Result<PathBuf, String> {
    let current_dir = std::env::current_dir()
        .map_err(|error| format!("failed to get current directory: {error}"))?;
    let output = Command::new("git")
        .arg("-C")
        .arg(&current_dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| {
            format!(
                "failed to discover Git repository root from {}: {error}",
                current_dir.display()
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "failed to discover Git repository root from {}: {}",
            current_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        format!("git rev-parse --show-toplevel returned non-UTF-8 output: {error}")
    })?;
    let path = stdout.trim();
    if path.is_empty() {
        return Err("git rev-parse --show-toplevel returned an empty path".to_owned());
    }

    canonical_repo_root(Path::new(path))
}

fn canonical_repo_root(path: &Path) -> Result<PathBuf, String> {
    std::fs::canonicalize(path).map_err(|error| {
        format!(
            "failed to resolve repository path {}: {error}",
            path.display()
        )
    })
}

fn merge_commit_in_progress(repo_root: &Path) -> Result<bool, String> {
    Ok(!merge_head_commits(repo_root)?.is_empty())
}

fn remote_merge_commit_in_progress(repo_root: &Path) -> Result<bool, String> {
    for commit in merge_head_commits(repo_root)? {
        if commit_reachable_from_remote_ref(repo_root, &commit)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn merge_head_commits(repo_root: &Path) -> Result<Vec<String>, String> {
    let merge_head = merge_head_path(repo_root)?;
    if !merge_head.exists() {
        return Ok(Vec::new());
    }

    let contents = std::fs::read_to_string(&merge_head).map_err(|error| {
        format!(
            "failed to read Git MERGE_HEAD path {}: {error}",
            merge_head.display()
        )
    })?;

    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

fn merge_head_path(repo_root: &Path) -> Result<PathBuf, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--git-path", "MERGE_HEAD"])
        .output()
        .map_err(|error| {
            format!(
                "failed to resolve Git MERGE_HEAD path for {}: {error}",
                repo_root.display()
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "failed to resolve Git MERGE_HEAD path for {}: {}",
            repo_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git rev-parse --git-path returned non-UTF-8 output: {error}"))?;
    let git_path = stdout.trim();
    if git_path.is_empty() {
        return Err("git rev-parse --git-path MERGE_HEAD returned an empty path".to_owned());
    }

    let merge_head = PathBuf::from(git_path);
    let merge_head = if merge_head.is_absolute() {
        merge_head
    } else {
        repo_root.join(merge_head)
    };

    Ok(merge_head)
}

fn commit_reachable_from_remote_ref(repo_root: &Path, commit: &str) -> Result<bool, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["for-each-ref", "--format=%(refname)", "--contains", commit])
        .arg("refs/remotes")
        .output()
        .map_err(|error| {
            format!(
                "failed to inspect remote refs for merge commit {commit} in {}: {error}",
                repo_root.display()
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "failed to inspect remote refs for merge commit {commit} in {}: {}",
            repo_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git for-each-ref returned non-UTF-8 output: {error}"))?;
    Ok(!stdout.trim().is_empty())
}

fn skip_merge_commit(format: OutputFormat, mode: MergeCommitSkipMode) -> i32 {
    eprintln!(
        "prec-bsl хук: пропущен merge commit ({})",
        merge_commit_skip_option(mode)
    );
    let report = PipelineReport {
        mode: PipelineMode::Hook,
        processed_files: Vec::new(),
        results: Vec::new(),
    };
    print!("{}", render_report(&report, format));
    0
}

fn merge_commit_skip_option(mode: MergeCommitSkipMode) -> &'static str {
    match mode {
        MergeCommitSkipMode::None => "",
        MergeCommitSkipMode::Any => "--ignore-merge-commit",
        MergeCommitSkipMode::Remote => "--ignore-remote-merge-commit",
    }
}

fn print_error(message: String) -> i32 {
    eprintln!("ошибка: {message}");
    1
}

/// `check`/`fix` contract: execution errors exit with `2`, keeping `1` for
/// "violations found" / "fixes applied".
fn print_execution_error(message: String) -> i32 {
    eprintln!("ошибка: {message}");
    2
}
