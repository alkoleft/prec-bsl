use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::config::ResolvedConfig;
use crate::git_index::StagedStatus;
use crate::scenarios::{
    REFERENCE_SCENARIOS, ScenarioDefinition, ScenarioSupport, find_reference_scenario,
    normalize_scenario_id,
};
use crate::source_files::{SourceFile, SourceRoot, classify_repo_path};
use crate::text_fixers::{
    CANONICAL_SPELLING_RULE, EXTRA_BLANK_LINES_RULE, KEYWORD_SPACING_RULE,
    TRAILING_WHITESPACE_RULE, canonical_spelling, extra_blank_lines, keyword_spacing,
    trailing_whitespace,
};

pub type ScenarioHandler = fn(&ScenarioExecutionContext<'_>) -> ScenarioRun;

#[derive(Debug, Clone)]
pub struct ScenarioRegistry {
    scenarios: BTreeMap<String, RegisteredScenario>,
}

impl ScenarioRegistry {
    pub fn reference() -> Self {
        let scenarios = REFERENCE_SCENARIOS
            .iter()
            .filter(|scenario| scenario.support == ScenarioSupport::RequiredV1)
            .map(|scenario| {
                (
                    scenario.id.to_owned(),
                    RegisteredScenario {
                        id: scenario.id.to_owned(),
                        definition: Some(scenario),
                        handler: reference_handler_for(scenario.id),
                        handles_deleted_files: false,
                    },
                )
            })
            .collect();

        Self { scenarios }
    }

    pub fn empty() -> Self {
        Self {
            scenarios: BTreeMap::new(),
        }
    }

    pub fn with_handler(mut self, scenario_id: &str, handler: ScenarioHandler) -> Self {
        let normalized = normalize_scenario_id(scenario_id).to_owned();
        let definition = find_reference_scenario(&normalized);
        self.scenarios.insert(
            normalized.clone(),
            RegisteredScenario {
                id: normalized,
                definition,
                handler,
                handles_deleted_files: false,
            },
        );
        self
    }

    pub fn with_deleted_file_handler(
        mut self,
        scenario_id: &str,
        handler: ScenarioHandler,
    ) -> Self {
        let normalized = normalize_scenario_id(scenario_id).to_owned();
        let definition = find_reference_scenario(&normalized);
        self.scenarios.insert(
            normalized.clone(),
            RegisteredScenario {
                id: normalized,
                definition,
                handler,
                handles_deleted_files: true,
            },
        );
        self
    }

    pub fn get(&self, scenario_id: &str) -> Option<&RegisteredScenario> {
        self.scenarios.get(normalize_scenario_id(scenario_id))
    }
}

impl Default for ScenarioRegistry {
    fn default() -> Self {
        Self::reference()
    }
}

#[derive(Debug, Clone)]
pub struct RegisteredScenario {
    pub id: String,
    pub definition: Option<&'static ScenarioDefinition>,
    handler: ScenarioHandler,
    handles_deleted_files: bool,
}

#[derive(Debug, Clone)]
pub struct ScenarioExecutionContext<'a> {
    pub repo_root: &'a Path,
    pub rule_id: &'a str,
    pub file: &'a SourceFile,
    pub settings: Option<&'a Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRun {
    pub results: Vec<ScenarioResult>,
    pub post_processing_paths: Vec<PathBuf>,
}

impl ScenarioRun {
    pub fn clean() -> Self {
        Self {
            results: Vec::new(),
            post_processing_paths: Vec::new(),
        }
    }

    pub fn single(result: ScenarioResult) -> Self {
        Self {
            results: vec![result],
            post_processing_paths: Vec::new(),
        }
    }

    pub fn with_post_processing_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.post_processing_paths.push(path.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioResult {
    pub rule_id: String,
    pub path: PathBuf,
    pub status: ScenarioResultStatus,
    pub message: String,
    pub modified_paths: Vec<PathBuf>,
    pub source_span: Option<SourceSpan>,
}

impl ScenarioResult {
    pub fn new(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        status: ScenarioResultStatus,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            path: path.into(),
            status,
            message: message.into(),
            modified_paths: Vec::new(),
            source_span: None,
        }
    }

    pub fn modified(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        let path = path.into();
        Self {
            rule_id: rule_id.into(),
            path: path.clone(),
            status: ScenarioResultStatus::Modified,
            message: message.into(),
            modified_paths: vec![path],
            source_span: None,
        }
    }

    pub fn warning(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, path, ScenarioResultStatus::Warning, message)
    }

    pub fn hard_failure(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, path, ScenarioResultStatus::HardFailure, message)
    }

    pub fn skipped(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, path, ScenarioResultStatus::Skipped, message)
    }

    pub fn unsupported(
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, path, ScenarioResultStatus::Unsupported, message)
    }

    pub fn with_modified_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.modified_paths.push(path.into());
        self
    }

    pub fn with_source_span(mut self, source_span: SourceSpan) -> Self {
        self.source_span = Some(source_span);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start_byte: usize,
    pub end_byte: usize,
}

impl SourceSpan {
    pub fn new(start_byte: usize, end_byte: usize) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioResultStatus {
    Modified,
    Warning,
    HardFailure,
    Skipped,
    Unsupported,
}

impl ScenarioResultStatus {
    pub fn is_blocking(self) -> bool {
        matches!(self, Self::HardFailure | Self::Unsupported)
    }

    pub fn is_modification(self) -> bool {
        self == Self::Modified
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineMode {
    Hook,
    ExecRules,
}

#[derive(Debug, Clone)]
pub struct PipelineRequest<'a> {
    pub repo_root: &'a Path,
    pub source_roots: &'a [SourceRoot],
    pub config: &'a ResolvedConfig,
    pub files: Vec<SourceFile>,
    pub mode: PipelineMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineReport {
    pub mode: PipelineMode,
    pub processed_files: Vec<PathBuf>,
    pub results: Vec<ScenarioResult>,
}

impl PipelineReport {
    pub fn critical_results(&self) -> Vec<&ScenarioResult> {
        self.results
            .iter()
            .filter(|result| result.status.is_blocking())
            .collect()
    }

    pub fn modified_paths(&self) -> Vec<PathBuf> {
        let mut paths = BTreeSet::new();
        for result in &self.results {
            if result.status.is_modification() {
                paths.insert(result.path.clone());
            }
            paths.extend(result.modified_paths.iter().cloned());
        }
        paths.into_iter().collect()
    }

    pub fn has_blocking_diagnostics(&self) -> bool {
        self.results
            .iter()
            .any(|result| result.status.is_blocking())
    }

    pub fn has_unreviewed_modifications(&self) -> bool {
        !self.modified_paths().is_empty()
    }

    pub fn hook_exit_code(&self) -> i32 {
        if self.has_blocking_diagnostics() || self.has_unreviewed_modifications() {
            1
        } else {
            0
        }
    }

    pub fn exec_rules_exit_code(&self) -> i32 {
        if self.has_blocking_diagnostics() {
            1
        } else {
            0
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self.mode {
            PipelineMode::Hook => self.hook_exit_code(),
            PipelineMode::ExecRules => self.exec_rules_exit_code(),
        }
    }
}

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

fn skipped_until_implemented(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    ScenarioRun::single(ScenarioResult::hard_failure(
        context.rule_id,
        context.file.repo_path.clone(),
        "scenario implementation is not available yet",
    ))
}

fn reference_handler_for(scenario_id: &str) -> ScenarioHandler {
    match scenario_id {
        TRAILING_WHITESPACE_RULE => trailing_whitespace,
        EXTRA_BLANK_LINES_RULE => extra_blank_lines,
        KEYWORD_SPACING_RULE => keyword_spacing,
        CANONICAL_SPELLING_RULE => canonical_spelling,
        _ => skipped_until_implemented,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use crate::config::parse_config_str;
    use crate::git_index::StagedStatus;
    use crate::source_files::{classify_repo_path, resolve_source_roots};

    const TRAILING_WHITESPACE: &str = "УдалениеЛишнихКонцевыхПробелов";
    const EXTRA_BLANK_LINES: &str = "УдалениеЛишнихПустыхСтрок";

    #[test]
    fn scenario_pipeline_keeps_configured_order_with_normalized_ids() {
        let repo = temp_repo("configured_order");
        write_file(repo.join("src/Модуль.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": [
                        "УдалениеЛишнихКонцевыхПробелов.os",
                        "ПроверкаНецензурныхСлов",
                        "УдалениеЛишнихПустыхСтрок.os"
                    ],
                    "ОтключенныеСценарии": ["ПроверкаНецензурныхСлов.os"]
                }
            }"#,
        )
        .unwrap();

        let registry = ScenarioRegistry::reference()
            .with_handler(TRAILING_WHITESPACE, hard_failure)
            .with_handler(EXTRA_BLANK_LINES, hard_failure);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::ExecRules,
            },
        );

        let executed_rules = report
            .results
            .iter()
            .map(|result| result.rule_id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(executed_rules, vec![TRAILING_WHITESPACE, EXTRA_BLANK_LINES]);
    }

    #[test]
    fn scenario_pipeline_uses_project_specific_scenario_order() {
        let repo = temp_repo("project_order");
        write_file(repo.join("src/Модуль.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "Проекты": {
                        "src": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let registry = ScenarioRegistry::reference().with_handler(EXTRA_BLANK_LINES, hard_failure);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::ExecRules,
            },
        );

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].rule_id, EXTRA_BLANK_LINES);
    }

    #[test]
    fn scenario_pipeline_appends_post_processing_files_to_queue_once() {
        let repo = temp_repo("post_processing_queue");
        write_file(repo.join("src/input.bsl"), "");
        write_file(repo.join("src/generated.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, "src/input.bsl", None).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();
        let registry =
            ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, append_generated_once);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::ExecRules,
            },
        );

        assert_eq!(
            report.processed_files,
            vec![
                PathBuf::from("src/input.bsl"),
                PathBuf::from("src/generated.bsl")
            ]
        );
    }

    #[test]
    fn scenario_pipeline_distinguishes_result_statuses_and_hook_exit() {
        let repo = temp_repo("statuses_and_exit");
        write_file(repo.join("src/Модуль.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();
        let registry =
            ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, all_statuses);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::Hook,
            },
        );

        let statuses = report
            .results
            .iter()
            .map(|result| result.status)
            .collect::<Vec<_>>();

        assert_eq!(
            statuses,
            vec![
                ScenarioResultStatus::Modified,
                ScenarioResultStatus::Warning,
                ScenarioResultStatus::HardFailure,
                ScenarioResultStatus::Skipped,
            ]
        );
        assert_eq!(report.critical_results().len(), 1);
        assert_eq!(
            report.modified_paths(),
            vec![PathBuf::from("src/Модуль.bsl")]
        );
        assert_eq!(report.hook_exit_code(), 1);
    }

    #[test]
    fn scenario_pipeline_accumulates_critical_errors_after_traversal() {
        let repo = temp_repo("critical_after_traversal");
        write_file(repo.join("src/one.bsl"), "");
        write_file(repo.join("src/two.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let files = ["src/one.bsl", "src/two.bsl"]
            .into_iter()
            .map(|path| classify_repo_path(&roots, path, None).unwrap())
            .collect::<Vec<_>>();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();
        let registry =
            ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, hard_failure);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files,
                mode: PipelineMode::ExecRules,
            },
        );

        assert_eq!(
            report.processed_files,
            vec![PathBuf::from("src/one.bsl"), PathBuf::from("src/two.bsl")]
        );
        assert_eq!(report.critical_results().len(), 2);
        assert_eq!(report.exec_rules_exit_code(), 1);
    }

    #[test]
    fn scenario_pipeline_reports_unregistered_enabled_scenario_as_unsupported() {
        let repo = temp_repo("unregistered");
        write_file(repo.join("src/Модуль.bsl"), "");
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file = classify_repo_path(&roots, "src/Модуль.bsl", None).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();

        let report = run_pipeline(
            &ScenarioRegistry::empty(),
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::ExecRules,
            },
        );

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].status, ScenarioResultStatus::Unsupported);
        assert!(
            report.results[0]
                .message
                .contains("scenario is not registered")
        );
    }

    #[test]
    fn scenario_pipeline_skips_deleted_files_without_deleted_file_capability() {
        let repo = temp_repo("deleted_file_skip");
        fs::create_dir_all(repo.join("src")).unwrap();
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file =
            classify_repo_path(&roots, "src/Удаленный.bsl", Some(StagedStatus::Deleted)).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();
        let registry =
            ScenarioRegistry::reference().with_handler(TRAILING_WHITESPACE, hard_failure);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::Hook,
            },
        );

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].status, ScenarioResultStatus::Skipped);
        assert_eq!(
            report.results[0].message,
            "scenario does not handle deleted files"
        );
    }

    #[test]
    fn scenario_pipeline_passes_deleted_files_to_explicit_deleted_file_handlers() {
        let repo = temp_repo("deleted_file_handler");
        fs::create_dir_all(repo.join("src")).unwrap();
        let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;
        let file =
            classify_repo_path(&roots, "src/Удаленный.bsl", Some(StagedStatus::Deleted)).unwrap();
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap();
        let registry = ScenarioRegistry::reference()
            .with_deleted_file_handler(TRAILING_WHITESPACE, hard_failure);

        let report = run_pipeline(
            &registry,
            PipelineRequest {
                repo_root: &repo,
                source_roots: &roots,
                config: &config,
                files: vec![file],
                mode: PipelineMode::Hook,
            },
        );

        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].status, ScenarioResultStatus::HardFailure);
        assert_eq!(report.hook_exit_code(), 1);
    }

    fn append_generated_once(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
        if context.file.repo_path == Path::new("src/input.bsl") {
            ScenarioRun::clean().with_post_processing_path("src/generated.bsl")
        } else {
            ScenarioRun::clean()
        }
    }

    fn all_statuses(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
        ScenarioRun {
            results: vec![
                ScenarioResult::modified(
                    context.rule_id,
                    context.file.repo_path.clone(),
                    "modified",
                ),
                ScenarioResult::warning(context.rule_id, context.file.repo_path.clone(), "warning"),
                ScenarioResult::hard_failure(
                    context.rule_id,
                    context.file.repo_path.clone(),
                    "hard failure",
                ),
                ScenarioResult::skipped(context.rule_id, context.file.repo_path.clone(), "skip"),
            ],
            post_processing_paths: Vec::new(),
        }
    }

    fn hard_failure(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
        ScenarioRun::single(ScenarioResult::hard_failure(
            context.rule_id,
            context.file.repo_path.clone(),
            "hard failure",
        ))
    }

    fn temp_repo(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .expect("current dir must be available")
            .join("target")
            .join("scenario-pipeline-tests")
            .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
        fs::create_dir_all(&path).expect("temporary scenario-pipeline test repo must be created");
        path
    }

    fn write_file(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}
