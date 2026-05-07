use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::ConfigError;
use crate::model::{
    ConfigSource, GlobalConfig, RepositoryScenarioSettings, ResolvedConfig, ScenarioConfig,
};
use crate::path::resolve_repo_path;
use crate::raw::parse_raw_config;
use crate::validation::{add_validation_warnings, validate_config};
use prec_bsl_scenarios::{ScenarioSupport, normalize_scenario_id};

pub(crate) const CONFIG_FILE_NAME: &str = "v8config.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigResolveRequest {
    pub repo_root: PathBuf,
    pub config_path: Option<PathBuf>,
    pub rule_override: Option<String>,
}

impl ConfigResolveRequest {
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
            config_path: None,
            rule_override: None,
        }
    }
}

pub fn resolve_config(request: &ConfigResolveRequest) -> Result<ResolvedConfig, ConfigError> {
    let (source, mut config) = match &request.config_path {
        Some(config_path) => {
            let path = resolve_repo_path(&request.repo_root, config_path);
            (
                ConfigSource::Explicit(path.clone()),
                load_config_from_path(&path)?,
            )
        }
        None => {
            let path = request.repo_root.join(CONFIG_FILE_NAME);
            if path.is_file() {
                (
                    ConfigSource::RepositoryDefault(path.clone()),
                    load_config_from_path(&path)?,
                )
            } else {
                (ConfigSource::BuiltInDefaults, built_in_defaults())
            }
        }
    };

    if let Some(rule_override) = &request.rule_override {
        let scenarios = parse_scenario_list(rule_override);
        config.scenarios.global_scenarios = scenarios.clone();
        config.scenarios.disabled_scenarios.clear();
        for project in config.scenarios.projects.values_mut() {
            project.global_scenarios = Some(scenarios.clone());
            project.disabled_scenarios = Some(Vec::new());
        }
    }

    add_validation_warnings(&mut config);
    validate_config(&config)?;
    config.source = source;

    Ok(config)
}

pub fn parse_config_str(source: &str) -> Result<ResolvedConfig, ConfigError> {
    let raw = parse_raw_config(source, None)?;
    let mut config = raw.into_resolved(ConfigSource::BuiltInDefaults);
    add_validation_warnings(&mut config);
    validate_config(&config)?;
    Ok(config)
}

pub fn built_in_defaults() -> ResolvedConfig {
    ResolvedConfig {
        source: ConfigSource::BuiltInDefaults,
        global: GlobalConfig {
            version: Some("2.0".to_owned()),
            edt_format: false,
            platform_version: None,
        },
        scenarios: ScenarioConfig {
            repository_scenarios: RepositoryScenarioSettings {
                use_repository_scenarios: false,
                local_scenarios_dir: None,
            },
            global_scenarios: prec_bsl_scenarios::REFERENCE_SCENARIOS
                .iter()
                .filter(|scenario| scenario.support == ScenarioSupport::RequiredV1)
                .map(|scenario| scenario.id.to_owned())
                .collect(),
            disabled_scenarios: Vec::new(),
            settings: BTreeMap::new(),
            projects: BTreeMap::new(),
        },
        warnings: Vec::new(),
    }
}

fn load_config_from_path(path: &Path) -> Result<ResolvedConfig, ConfigError> {
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let raw = parse_raw_config(&source, Some(path))?;

    Ok(raw.into_resolved(ConfigSource::RepositoryDefault(path.to_path_buf())))
}

fn parse_scenario_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(normalize_scenario_id)
        .filter(|scenario| !scenario.is_empty())
        .map(str::to_owned)
        .collect()
}
