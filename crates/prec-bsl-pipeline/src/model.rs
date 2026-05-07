use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use prec_bsl_config::ResolvedConfig;
use prec_bsl_source::{SourceFile, SourceRoot};
use serde_json::Value;

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
