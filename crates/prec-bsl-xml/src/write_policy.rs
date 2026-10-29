use std::fs;
use std::path::Path;

use prec_bsl_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun, WritePolicy};

/// Shared write boundary of XML/EDT fixers: honors [`WritePolicy`] so the
/// `check` command reports would-be modifications without touching the working
/// tree.
pub fn apply_text_modification(
    context: &ScenarioExecutionContext<'_>,
    path: &Path,
    output: String,
    modified_message: &str,
) -> ScenarioRun {
    if context.write_policy == WritePolicy::DryRun {
        return ScenarioRun::single(ScenarioResult::modified_dry_run(
            context.rule_id,
            context.file.repo_path.clone(),
            modified_message,
            output,
        ));
    }

    if let Err(error) = fs::write(path, output) {
        return ScenarioRun::single(ScenarioResult::hard_failure(
            context.rule_id,
            context.file.repo_path.clone(),
            format!("failed to write file: {error}"),
        ));
    }

    ScenarioRun::single(ScenarioResult::modified(
        context.rule_id,
        context.file.repo_path.clone(),
        modified_message,
    ))
}
