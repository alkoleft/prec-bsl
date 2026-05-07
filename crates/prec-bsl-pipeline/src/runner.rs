use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::model::{PipelineReport, PipelineRequest, ScenarioExecutionContext, ScenarioResult};
use crate::registry::ScenarioRegistry;
use prec_bsl_git::StagedStatus;
use prec_bsl_scenarios::normalize_scenario_id;
use prec_bsl_source::{SourceFile, SourceRoot, classify_repo_path};

pub fn run_pipeline(registry: &ScenarioRegistry, request: PipelineRequest<'_>) -> PipelineReport {
    let mut results = Vec::new();
    let mut processed_files = Vec::new();
    let mut queue = VecDeque::from(request.files);
    let mut queued_or_processed = queue
        .iter()
        .map(|file| file.repo_path.clone())
        .collect::<BTreeSet<_>>();

    while let Some(file) = queue.pop_front() {
        processed_files.push(file.repo_path.clone());
        let scenario_ids = request.config.enabled_scenarios_for_path(&file.repo_path);
        for scenario_id in scenario_ids {
            let normalized = normalize_scenario_id(&scenario_id);
            let settings = request
                .config
                .scenario_settings_for_path(&file.repo_path, normalized);
            let Some(scenario) = registry.get(normalized) else {
                results.push(ScenarioResult::unsupported(
                    normalized,
                    file.repo_path.clone(),
                    format!("scenario is not registered: {normalized}"),
                ));
                continue;
            };
            if is_deleted_file(&file) && !scenario.handles_deleted_files {
                results.push(ScenarioResult::skipped(
                    normalized,
                    file.repo_path.clone(),
                    "scenario does not handle deleted files",
                ));
                continue;
            }

            let context = ScenarioExecutionContext {
                repo_root: request.repo_root,
                rule_id: &scenario.id,
                file: &file,
                settings,
            };
            let run = (scenario.handler)(&context);
            for path in run.post_processing_paths {
                append_post_processing_file(
                    request.source_roots,
                    normalized,
                    &file.repo_path,
                    path,
                    &mut queued_or_processed,
                    &mut queue,
                    &mut results,
                );
            }
            results.extend(run.results);
        }
    }

    PipelineReport {
        mode: request.mode,
        processed_files,
        results,
    }
}

fn is_deleted_file(file: &SourceFile) -> bool {
    matches!(file.staged_status, Some(StagedStatus::Deleted))
}

fn append_post_processing_file(
    source_roots: &[SourceRoot],
    rule_id: &str,
    current_file: &Path,
    path: PathBuf,
    queued_or_processed: &mut BTreeSet<PathBuf>,
    queue: &mut VecDeque<SourceFile>,
    results: &mut Vec<ScenarioResult>,
) {
    if !queued_or_processed.insert(path.clone()) {
        return;
    }

    if let Some(source_file) = classify_repo_path(source_roots, path.clone(), None) {
        queue.push_back(source_file);
    } else {
        results.push(ScenarioResult::warning(
            rule_id,
            current_file.to_path_buf(),
            format!(
                "post-processing path is outside configured source roots: {}",
                path.display()
            ),
        ));
    }
}
