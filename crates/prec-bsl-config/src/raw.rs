use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use crate::error::ConfigError;
use crate::model::{
    ConfigSource, ConfigWarning, GlobalConfig, ProjectScenarioConfig, RepositoryScenarioSettings,
    ResolvedConfig, ScenarioConfig,
};
use crate::path::{empty_string_as_none, normalize_project_path};
use crate::scenario::normalize_scenario_id;
use crate::validation::credential_key_paths;

fn normalize_setting_map(settings: BTreeMap<String, Value>) -> BTreeMap<String, Value> {
    settings
        .into_iter()
        .map(|(scenario, value)| (normalize_scenario_id(&scenario).to_owned(), value))
        .collect()
}

fn normalize_optional_setting_map(
    settings: Option<BTreeMap<String, Value>>,
) -> Option<BTreeMap<String, Value>> {
    settings.map(normalize_setting_map)
}

fn normalize_scenario_vec(scenarios: Vec<String>) -> Vec<String> {
    scenarios
        .into_iter()
        .map(|scenario| normalize_scenario_id(&scenario).to_owned())
        .collect()
}

fn normalize_optional_scenario_vec(scenarios: Option<Vec<String>>) -> Option<Vec<String>> {
    scenarios.map(normalize_scenario_vec)
}
pub(crate) fn parse_raw_config(
    source: &str,
    path: Option<&Path>,
) -> Result<RawConfig, ConfigError> {
    let value = serde_json::from_str::<Value>(source).map_err(|source| ConfigError::Json {
        path: path.map(Path::to_path_buf),
        source,
    })?;

    let credential_paths = credential_key_paths(&value);
    if !credential_paths.is_empty() {
        return Err(ConfigError::Validation {
            errors: credential_paths
                .into_iter()
                .map(|key_path| format!("credential key is not allowed in config: {key_path}"))
                .collect(),
        });
    }

    serde_json::from_value::<RawConfig>(value).map_err(|source| ConfigError::Json {
        path: path.map(Path::to_path_buf),
        source,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawConfig {
    #[serde(rename = "GLOBAL", default)]
    global: RawGlobalConfig,
    #[serde(rename = "Precommt4onecСценарии", default)]
    scenarios: RawScenarioConfig,
    #[serde(flatten)]
    unknown_top_level: BTreeMap<String, Value>,
}

impl RawConfig {
    pub(crate) fn into_resolved(self, source: ConfigSource) -> ResolvedConfig {
        let warnings = self
            .unknown_top_level
            .into_keys()
            .map(|key| ConfigWarning {
                message: format!("unknown top-level config key: {key}"),
            })
            .collect();

        ResolvedConfig {
            source,
            global: self.global.into_domain(),
            scenarios: self.scenarios.into_domain(),
            warnings,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawGlobalConfig {
    version: Option<String>,
    #[serde(rename = "ФорматEDT", default)]
    edt_format: bool,
    #[serde(rename = "ВерсияПлатформы")]
    platform_version: Option<String>,
}

impl RawGlobalConfig {
    fn into_domain(self) -> GlobalConfig {
        GlobalConfig {
            version: self.version,
            edt_format: self.edt_format,
            platform_version: self.platform_version.and_then(empty_string_as_none),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawScenarioConfig {
    #[serde(rename = "ИспользоватьСценарииРепозитория", default)]
    use_repository_scenarios: bool,
    #[serde(rename = "КаталогЛокальныхСценариев", default)]
    local_scenarios_dir: String,
    #[serde(rename = "ГлобальныеСценарии", default)]
    global_scenarios: Vec<String>,
    #[serde(rename = "ОтключенныеСценарии", default)]
    disabled_scenarios: Vec<String>,
    #[serde(rename = "НастройкиСценариев", default)]
    settings: BTreeMap<String, Value>,
    #[serde(rename = "Проекты", default)]
    projects: BTreeMap<String, RawProjectScenarioConfig>,
}

impl RawScenarioConfig {
    fn into_domain(self) -> ScenarioConfig {
        ScenarioConfig {
            repository_scenarios: RepositoryScenarioSettings {
                use_repository_scenarios: self.use_repository_scenarios,
                local_scenarios_dir: empty_string_as_none(self.local_scenarios_dir),
            },
            global_scenarios: normalize_scenario_vec(self.global_scenarios),
            disabled_scenarios: normalize_scenario_vec(self.disabled_scenarios),
            settings: normalize_setting_map(self.settings),
            projects: self
                .projects
                .into_iter()
                .map(|(path, project)| {
                    (
                        normalize_project_path(&path),
                        project.into_domain(path.to_owned()),
                    )
                })
                .collect(),
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawProjectScenarioConfig {
    #[serde(rename = "ИспользоватьСценарииРепозитория", default)]
    use_repository_scenarios: bool,
    #[serde(rename = "КаталогЛокальныхСценариев", default)]
    local_scenarios_dir: String,
    #[serde(rename = "ГлобальныеСценарии")]
    global_scenarios: Option<Vec<String>>,
    #[serde(rename = "ОтключенныеСценарии")]
    disabled_scenarios: Option<Vec<String>>,
    #[serde(rename = "НастройкиСценариев")]
    settings: Option<BTreeMap<String, Value>>,
}

impl RawProjectScenarioConfig {
    fn into_domain(self, configured_path: String) -> ProjectScenarioConfig {
        ProjectScenarioConfig {
            configured_path,
            repository_scenarios: RepositoryScenarioSettings {
                use_repository_scenarios: self.use_repository_scenarios,
                local_scenarios_dir: empty_string_as_none(self.local_scenarios_dir),
            },
            global_scenarios: normalize_optional_scenario_vec(self.global_scenarios),
            disabled_scenarios: normalize_optional_scenario_vec(self.disabled_scenarios),
            settings: normalize_optional_setting_map(self.settings),
        }
    }
}
