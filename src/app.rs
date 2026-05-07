use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::{ExecRulesArgs, PrekHookArgs};
use crate::config::{ConfigResolveRequest, resolve_config};
use crate::git_index::{collect_staged_files, restage_paths};
use crate::output::{OutputFormat, render_report};
use crate::reference_registry;
use crate::scenario_pipeline::{
    PipelineMode, PipelineReport, PipelineRequest, ScenarioResult, run_pipeline,
};
use crate::source_files::{
    classify_staged_files, collect_source_files, parse_source_dir_list, resolve_source_roots,
};

pub fn run_prek_hook(args: &PrekHookArgs) -> i32 {
    let repo_root = match current_repo_root() {
        Ok(repo_root) => repo_root,
        Err(message) => return print_error(message),
    };

    let source_dirs = args.source_dir.iter().cloned().collect::<Vec<_>>();
    let Some((config, roots)) = resolve_config_and_roots(
        &repo_root,
        args.config.clone(),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        &source_dirs,
        PipelineMode::Hook,
        args.format,
    ) else {
        return 1;
    };

    let staged_files = match collect_staged_files(&repo_root) {
        Ok(staged_files) => staged_files,
        Err(error) => return print_error(error.to_string()),
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
    let repo_root = match canonical_repo_root(&args.repo) {
        Ok(repo_root) => repo_root,
        Err(message) => return print_error(message),
    };
    let source_dirs = args
        .source_dirs
        .as_ref()
        .map(|source_dirs| parse_source_dir_list(&source_dirs.0))
        .unwrap_or_default();
    let Some((config, roots)) = resolve_config_and_roots(
        &repo_root,
        args.config.clone(),
        args.rules.as_ref().map(|rules| rules.0.clone()),
        &source_dirs,
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

fn resolve_config_and_roots(
    repo_root: &Path,
    config_path: Option<PathBuf>,
    rule_override: Option<String>,
    source_dirs: &[PathBuf],
    mode: PipelineMode,
    format: OutputFormat,
) -> Option<(
    crate::config::ResolvedConfig,
    Vec<crate::source_files::SourceRoot>,
)> {
    let mut config_request = ConfigResolveRequest::new(repo_root);
    config_request.config_path = config_path;
    config_request.rule_override = rule_override;
    let config = match resolve_config(&config_request) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("error: {error}");
            return None;
        }
    };

    let resolution = resolve_source_roots(repo_root, source_dirs);
    if !resolution.diagnostics.is_empty() {
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
        return None;
    }

    Some((config, resolution.roots))
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

fn print_error(message: String) -> i32 {
    eprintln!("error: {message}");
    1
}
