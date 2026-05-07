use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::path::{normalize_relative_path, path_matches_project};
use prec_bsl_scenarios::normalize_scenario_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigSource {
    Explicit(PathBuf),
    RepositoryDefault(PathBuf),
    BuiltInDefaults,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedConfig {
    pub source: ConfigSource,
    pub global: GlobalConfig,
    pub scenarios: ScenarioConfig,
    pub warnings: Vec<ConfigWarning>,
}

impl ResolvedConfig {
    pub fn enabled_scenarios_for_path(&self, source_path: &Path) -> Vec<String> {
        let scenarios = self.scenarios.for_source_path(source_path);
        scenarios.enabled_scenarios_owned()
    }

    pub fn scenario_settings_for_path(
        &self,
        source_path: &Path,
        scenario_id: &str,
    ) -> Option<&Value> {
        let normalized = normalize_scenario_id(scenario_id);
        let scenarios = self.scenarios.for_source_path(source_path);
        scenarios
            .settings
            .and_then(|settings| settings.get(normalized))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalConfig {
    pub version: Option<String>,
    pub edt_format: bool,
    pub platform_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioConfig {
    pub repository_scenarios: RepositoryScenarioSettings,
    pub global_scenarios: Vec<String>,
    pub disabled_scenarios: Vec<String>,
    pub settings: BTreeMap<String, Value>,
    pub projects: BTreeMap<String, ProjectScenarioConfig>,
}

impl ScenarioConfig {
    pub fn enabled_scenarios(&self) -> Vec<&str> {
        self.global_scenarios
            .iter()
            .filter(|scenario| !self.is_disabled(scenario))
            .map(String::as_str)
            .collect()
    }

    fn is_disabled(&self, scenario_id: &str) -> bool {
        let normalized = normalize_scenario_id(scenario_id);
        self.disabled_scenarios
            .iter()
            .any(|disabled| disabled == normalized)
    }

    fn for_source_path<'a>(&'a self, source_path: &Path) -> ScenarioView<'a> {
        self.matching_project(source_path)
            .map(|project| ScenarioView {
                global_scenarios: project
                    .global_scenarios
                    .as_ref()
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                disabled_scenarios: project
                    .disabled_scenarios
                    .as_ref()
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                settings: project.settings.as_ref(),
            })
            .unwrap_or(ScenarioView {
                global_scenarios: &self.global_scenarios,
                disabled_scenarios: &self.disabled_scenarios,
                settings: Some(&self.settings),
            })
    }

    fn matching_project(&self, source_path: &Path) -> Option<&ProjectScenarioConfig> {
        let source_path = normalize_relative_path(source_path);

        self.projects
            .iter()
            .filter(|(project_path, _project)| path_matches_project(&source_path, project_path))
            .max_by_key(|(project_path, _project)| project_path.len())
            .map(|(_project_path, project)| project)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryScenarioSettings {
    pub use_repository_scenarios: bool,
    pub local_scenarios_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectScenarioConfig {
    pub configured_path: String,
    pub repository_scenarios: RepositoryScenarioSettings,
    pub global_scenarios: Option<Vec<String>>,
    pub disabled_scenarios: Option<Vec<String>>,
    pub settings: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScenarioView<'a> {
    global_scenarios: &'a [String],
    disabled_scenarios: &'a [String],
    settings: Option<&'a BTreeMap<String, Value>>,
}

impl ScenarioView<'_> {
    fn enabled_scenarios_owned(&self) -> Vec<String> {
        self.global_scenarios
            .iter()
            .filter(|scenario| !self.is_disabled(scenario))
            .cloned()
            .collect()
    }

    fn is_disabled(&self, scenario_id: &str) -> bool {
        let normalized = normalize_scenario_id(scenario_id);
        self.disabled_scenarios
            .iter()
            .any(|disabled| disabled == normalized)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigWarning {
    pub message: String,
}
