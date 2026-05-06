use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::scenarios::{ScenarioSupport, find_reference_scenario, normalize_scenario_id};

const CONFIG_FILE_NAME: &str = "v8config.json";

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
struct ScenarioView<'a> {
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

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: Option<PathBuf>,
        source: serde_json::Error,
    },
    Validation {
        errors: Vec<String>,
    },
}

impl ConfigError {
    pub fn validation_messages(&self) -> &[String] {
        match self {
            Self::Validation { errors } => errors,
            Self::Io { .. } | Self::Json { .. } => &[],
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                if let Some(path) = path {
                    write!(formatter, "failed to parse {}: {source}", path.display())
                } else {
                    write!(formatter, "failed to parse config JSON: {source}")
                }
            }
            Self::Validation { errors } => {
                write!(formatter, "invalid configuration: {}", errors.join("; "))
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Validation { .. } => None,
        }
    }
}

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
            global_scenarios: crate::scenarios::REFERENCE_SCENARIOS
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

fn parse_raw_config(source: &str, path: Option<&Path>) -> Result<RawConfig, ConfigError> {
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

fn validate_config(config: &ResolvedConfig) -> Result<(), ConfigError> {
    let mut errors = Vec::new();

    validate_enabled_scenarios(&config.scenarios.global_scenarios, "global", &mut errors);
    validate_repository_path(
        config
            .scenarios
            .repository_scenarios
            .local_scenarios_dir
            .as_deref(),
        "КаталогЛокальныхСценариев",
        &mut errors,
    );
    for (project_path, project) in &config.scenarios.projects {
        if !is_repository_relative_path(&project.configured_path) {
            errors.push(format!(
                "project path must be repository-relative: {}",
                project.configured_path
            ));
        }
        validate_repository_path(
            project.repository_scenarios.local_scenarios_dir.as_deref(),
            &format!("Проекты.{project_path}.КаталогЛокальныхСценариев"),
            &mut errors,
        );
        if let Some(global_scenarios) = &project.global_scenarios {
            validate_enabled_scenarios(global_scenarios, project_path, &mut errors);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(ConfigError::Validation { errors })
    }
}

fn validate_repository_path(path: Option<&str>, scope: &str, errors: &mut Vec<String>) {
    if let Some(path) = path
        && !is_repository_relative_path(path)
    {
        errors.push(format!("{scope} must be repository-relative: {path}"));
    }
}

fn credential_key_paths(value: &Value) -> Vec<String> {
    let mut paths = Vec::new();
    collect_credential_key_paths(value, "$", &mut paths);
    paths
}

fn collect_credential_key_paths(value: &Value, path: &str, paths: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let key_path = format!("{path}.{key}");
                if is_credential_key(key) {
                    paths.push(key_path.clone());
                }
                collect_credential_key_paths(child, &key_path, paths);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_credential_key_paths(child, &format!("{path}[{index}]"), paths);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn is_credential_key(key: &str) -> bool {
    let key = key.to_lowercase();
    key.contains("password")
        || key.contains("passwd")
        || key.contains("credential")
        || key.contains("secret")
        || key.contains("token")
        || key.contains("username")
        || key.contains("login")
        || key.contains("пароль")
        || key.contains("секрет")
        || key.contains("токен")
        || key.contains("логин")
        || key.contains("пользовател")
        || key == "user"
        || key.ends_with("_user")
}

fn add_validation_warnings(config: &mut ResolvedConfig) {
    add_unknown_disabled_warnings(
        &config.scenarios.disabled_scenarios,
        "global",
        &mut config.warnings,
    );
    for (project_path, project) in &config.scenarios.projects {
        if let Some(disabled_scenarios) = &project.disabled_scenarios {
            add_unknown_disabled_warnings(disabled_scenarios, project_path, &mut config.warnings);
        }
    }
}

fn add_unknown_disabled_warnings(
    scenarios: &[String],
    scope: &str,
    warnings: &mut Vec<ConfigWarning>,
) {
    for scenario in scenarios {
        if find_reference_scenario(scenario).is_none() {
            warnings.push(ConfigWarning {
                message: format!(
                    "unknown disabled scenario id in {scope}: {}",
                    normalize_scenario_id(scenario)
                ),
            });
        }
    }
}

fn validate_enabled_scenarios(scenarios: &[String], scope: &str, errors: &mut Vec<String>) {
    for scenario in scenarios {
        let normalized = normalize_scenario_id(scenario);
        match find_reference_scenario(scenario).map(|definition| definition.support) {
            Some(ScenarioSupport::RequiredV1) => {}
            Some(ScenarioSupport::Unsupported) => errors.push(format!(
                "unsupported built-in scenario in v1 enabled in {scope}: {normalized}"
            )),
            None => errors.push(format!(
                "unsupported repository-local scenario in v1 enabled in {scope}: {normalized}; dynamic local .os execution is not supported in v1"
            )),
        }
    }
}

fn resolve_repo_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn looks_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute()
        || path.starts_with('\\')
        || path
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
}

fn is_repository_relative_path(path: &str) -> bool {
    !looks_absolute_path(path)
        && !path
            .replace('\\', "/")
            .split('/')
            .any(|component| component == "..")
}

fn parse_scenario_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(normalize_scenario_id)
        .filter(|scenario| !scenario.is_empty())
        .map(str::to_owned)
        .collect()
}

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

fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn path_matches_project(source_path: &str, project_path: &str) -> bool {
    source_path == project_path
        || source_path
            .strip_prefix(project_path)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn empty_string_as_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(rename = "GLOBAL", default)]
    global: RawGlobalConfig,
    #[serde(rename = "Precommt4onecСценарии", default)]
    scenarios: RawScenarioConfig,
    #[serde(flatten)]
    unknown_top_level: BTreeMap<String, Value>,
}

impl RawConfig {
    fn into_resolved(self, source: ConfigSource) -> ResolvedConfig {
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

fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_matches('/')
        .trim_start_matches("./")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;
    use crate::scenarios::UNSUPPORTED_ORDINARY_FORMS;

    #[test]
    fn config_explicit_path_overrides_default_discovery() {
        let repo = temp_repo("explicit_path_overrides_default_discovery");
        write_config(
            &repo.join(CONFIG_FILE_NAME),
            r#"{
                "GLOBAL": {"version": "default"},
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                }
            }"#,
        );
        write_config(
            &repo.join("custom-v8config.json"),
            r#"{
                "GLOBAL": {"version": "custom", "ФорматEDT": true},
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        );

        let mut request = ConfigResolveRequest::new(&repo);
        request.config_path = Some(PathBuf::from("custom-v8config.json"));

        let config = resolve_config(&request).unwrap();

        assert_eq!(
            config.source,
            ConfigSource::Explicit(repo.join("custom-v8config.json"))
        );
        assert_eq!(config.global.version.as_deref(), Some("custom"));
        assert!(config.global.edt_format);
        assert_eq!(
            config.scenarios.global_scenarios,
            vec!["УдалениеЛишнихКонцевыхПробелов"]
        );
    }

    #[test]
    fn config_default_discovery_checks_repo_root_v8config() {
        let repo = temp_repo("default_discovery_checks_repo_root_v8config");
        write_config(
            &repo.join(CONFIG_FILE_NAME),
            r#"{
                "GLOBAL": {"version": "repo-default"},
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["ПроверкаНецензурныхСлов.os"]
                }
            }"#,
        );

        let config = resolve_config(&ConfigResolveRequest::new(&repo)).unwrap();

        assert_eq!(
            config.source,
            ConfigSource::RepositoryDefault(repo.join(CONFIG_FILE_NAME))
        );
        assert_eq!(config.global.version.as_deref(), Some("repo-default"));
        assert_eq!(
            config.scenarios.global_scenarios,
            vec!["ПроверкаНецензурныхСлов"]
        );
    }

    #[test]
    fn config_built_in_defaults_are_available_without_file() {
        let repo = temp_repo("built_in_defaults_are_available_without_file");

        let config = resolve_config(&ConfigResolveRequest::new(&repo)).unwrap();

        assert_eq!(config.source, ConfigSource::BuiltInDefaults);
        assert!(
            config
                .scenarios
                .global_scenarios
                .contains(&"УдалениеЛишнихКонцевыхПробелов".to_owned())
        );
        assert!(
            !config
                .scenarios
                .global_scenarios
                .contains(&UNSUPPORTED_ORDINARY_FORMS.to_owned())
        );
    }

    #[test]
    fn config_parses_global_historic_key_and_all_scenario_layers() {
        let config = parse_config_str(
            r#"{
                "GLOBAL": {
                    "version": "2.1",
                    "ФорматEDT": true,
                    "ВерсияПлатформы": "8.3.20.1996"
                },
                "Precommt4onecСценарии": {
                    "ИспользоватьСценарииРепозитория": true,
                    "КаталогЛокальныхСценариев": "tools/pre-commit",
                    "ГлобальныеСценарии": [
                        "УдалениеЛишнихКонцевыхПробелов.os",
                        "ПроверкаНецензурныхСлов"
                    ],
                    "ОтключенныеСценарии": ["ЛокальныйОтключенный.os"],
                    "НастройкиСценариев": {
                        "ПроверкаНецензурныхСлов.os": {
                            "ФайлСНецензурнымиСловами": "НецензурныеСлова.txt"
                        }
                    },
                    "Проекты": {
                        "fixtures/configuration": {
                            "ИспользоватьСценарииРепозитория": false,
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"],
                            "ОтключенныеСценарии": [],
                            "НастройкиСценариев": {
                                "УдалениеЛишнихПустыхСтрок": {
                                    "Максимум": 1
                                }
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(config.global.version.as_deref(), Some("2.1"));
        assert!(config.global.edt_format);
        assert_eq!(
            config.global.platform_version.as_deref(),
            Some("8.3.20.1996")
        );
        assert!(
            config
                .scenarios
                .repository_scenarios
                .use_repository_scenarios
        );
        assert_eq!(
            config
                .scenarios
                .repository_scenarios
                .local_scenarios_dir
                .as_deref(),
            Some("tools/pre-commit")
        );
        assert_eq!(
            config.scenarios.global_scenarios,
            vec!["УдалениеЛишнихКонцевыхПробелов", "ПроверкаНецензурныхСлов"]
        );
        assert_eq!(
            config.scenarios.disabled_scenarios,
            vec!["ЛокальныйОтключенный"]
        );
        assert!(
            config
                .scenarios
                .settings
                .contains_key("ПроверкаНецензурныхСлов")
        );
        assert!(
            config
                .scenarios
                .projects
                .contains_key("fixtures/configuration")
        );
    }

    #[test]
    fn config_project_settings_override_base_for_matching_source_subpaths() {
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": [
                        "УдалениеЛишнихКонцевыхПробелов.os",
                        "ПроверкаНецензурныхСлов.os"
                    ],
                    "НастройкиСценариев": {
                        "ПроверкаНецензурныхСлов": {"Файл": "base.txt"}
                    },
                    "Проекты": {
                        "fixtures/configuration": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"],
                            "НастройкиСценариев": {
                                "ПроверкаНецензурныхСлов": {"Файл": "project.txt"}
                            }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            config.enabled_scenarios_for_path(Path::new("fixtures/configuration/src/Модуль.bsl")),
            vec!["УдалениеЛишнихПустыхСтрок"]
        );
        assert_eq!(
            config.scenario_settings_for_path(
                Path::new("fixtures/configuration/src/Модуль.bsl"),
                "ПроверкаНецензурныхСлов.os"
            ),
            Some(&json!({"Файл": "project.txt"}))
        );
        assert_eq!(
            config.scenario_settings_for_path(
                Path::new("exts/rat/src/Модуль.bsl"),
                "ПроверкаНецензурныхСлов"
            ),
            Some(&json!({"Файл": "base.txt"}))
        );
    }

    #[test]
    fn config_project_override_does_not_fall_back_to_base_disabled_or_settings() {
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "ОтключенныеСценарии": ["ПроверкаНецензурныхСлов.os"],
                    "НастройкиСценариев": {
                        "ПроверкаНецензурныхСлов": {"Файл": "base.txt"}
                    },
                    "Проекты": {
                        "configuration": {
                            "ГлобальныеСценарии": ["ПроверкаНецензурныхСлов.os"]
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            config.enabled_scenarios_for_path(Path::new("configuration/src/Модуль.bsl")),
            vec!["ПроверкаНецензурныхСлов"]
        );
        assert_eq!(
            config.scenario_settings_for_path(
                Path::new("configuration/src/Модуль.bsl"),
                "ПроверкаНецензурныхСлов"
            ),
            None
        );
    }

    #[test]
    fn config_cli_rule_override_replaces_enabled_scenarios() {
        let repo = temp_repo("cli_rule_override_replaces_enabled_scenarios");
        write_config(
            &repo.join(CONFIG_FILE_NAME),
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        );

        let mut request = ConfigResolveRequest::new(&repo);
        request.rule_override =
            Some("УдалениеЛишнихПустыхСтрок.os,ПроверкаНецензурныхСлов".to_owned());

        let config = resolve_config(&request).unwrap();

        assert_eq!(
            config.scenarios.global_scenarios,
            vec!["УдалениеЛишнихПустыхСтрок", "ПроверкаНецензурныхСлов"]
        );
    }

    #[test]
    fn config_cli_rule_override_takes_precedence_over_project_scenarios() {
        let repo = temp_repo("cli_rule_override_takes_precedence_over_project_scenarios");
        write_config(
            &repo.join(CONFIG_FILE_NAME),
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "Проекты": {
                        "configuration": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                        }
                    }
                }
            }"#,
        );

        let mut request = ConfigResolveRequest::new(&repo);
        request.rule_override = Some("ПроверкаНецензурныхСлов".to_owned());

        let config = resolve_config(&request).unwrap();

        assert_eq!(
            config.enabled_scenarios_for_path(Path::new("configuration/src/Модуль.bsl")),
            vec!["ПроверкаНецензурныхСлов"]
        );
    }

    #[test]
    fn config_cli_rule_override_takes_precedence_over_disabled_scenarios() {
        let repo = temp_repo("cli_rule_override_takes_precedence_over_disabled_scenarios");
        write_config(
            &repo.join(CONFIG_FILE_NAME),
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "ОтключенныеСценарии": ["ПроверкаНецензурныхСлов.os"],
                    "Проекты": {
                        "configuration": {
                            "ОтключенныеСценарии": ["ПроверкаНецензурныхСлов.os"]
                        }
                    }
                }
            }"#,
        );

        let mut request = ConfigResolveRequest::new(&repo);
        request.rule_override = Some("ПроверкаНецензурныхСлов".to_owned());

        let config = resolve_config(&request).unwrap();

        assert_eq!(
            config.enabled_scenarios_for_path(Path::new("configuration/src/Модуль.bsl")),
            vec!["ПроверкаНецензурныхСлов"]
        );
    }

    #[test]
    fn config_enabled_unsupported_ordinary_forms_fails_validation() {
        let error = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["РазборОбычныхФормНаИсходники.os"]
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &[
                "unsupported built-in scenario in v1 enabled in global: РазборОбычныхФормНаИсходники"
            ]
        );
    }

    #[test]
    fn config_enabled_repository_local_scenario_fails_validation() {
        let error = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["СортировкаДереваМетаданных.os"]
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &[
                "unsupported repository-local scenario in v1 enabled in global: СортировкаДереваМетаданных; dynamic local .os execution is not supported in v1"
            ]
        );
    }

    #[test]
    fn config_credential_keys_fail_validation() {
        let error = parse_config_str(
            r#"{
                "GLOBAL": {},
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "НастройкиСценариев": {
                        "УдалениеЛишнихКонцевыхПробелов": {
                            "password": "secret"
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &[
                "credential key is not allowed in config: $.Precommt4onecСценарии.НастройкиСценариев.УдалениеЛишнихКонцевыхПробелов.password"
            ]
        );
    }

    #[test]
    fn config_secret_token_and_user_keys_fail_validation() {
        let error = parse_config_str(
            r#"{
                "GLOBAL": {},
                "secret": "hidden",
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "НастройкиСценариев": {
                        "УдалениеЛишнихКонцевыхПробелов": {
                            "api_token": "token",
                            "ИмяПользователя": "user"
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert!(
            error
                .validation_messages()
                .contains(&"credential key is not allowed in config: $.secret".to_owned())
        );
        assert!(error.validation_messages().contains(
            &"credential key is not allowed in config: $.Precommt4onecСценарии.НастройкиСценариев.УдалениеЛишнихКонцевыхПробелов.api_token"
                .to_owned()
        ));
        assert!(error.validation_messages().contains(
            &"credential key is not allowed in config: $.Precommt4onecСценарии.НастройкиСценариев.УдалениеЛишнихКонцевыхПробелов.ИмяПользователя"
                .to_owned()
        ));
    }

    #[test]
    fn config_local_scenario_directory_must_be_repository_relative() {
        let error = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "КаталогЛокальныхСценариев": "/tmp/pre-commit",
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &["КаталогЛокальныхСценариев must be repository-relative: /tmp/pre-commit"]
        );
    }

    #[test]
    fn config_project_paths_must_be_repository_relative_before_normalization() {
        let error = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "Проекты": {
                        "/configuration": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &["project path must be repository-relative: /configuration"]
        );
    }

    #[test]
    fn config_paths_must_not_escape_repository_with_parent_components() {
        let error = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "КаталогЛокальныхСценариев": "../tools/pre-commit",
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "Проекты": {
                        "../configuration": {
                            "ГлобальныеСценарии": ["УдалениеЛишнихПустыхСтрок.os"]
                        }
                    }
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(
            error.validation_messages(),
            &[
                "КаталогЛокальныхСценариев must be repository-relative: ../tools/pre-commit",
                "project path must be repository-relative: ../configuration",
            ]
        );
    }

    #[test]
    fn config_disabled_unknown_scenarios_parse_compatibly() {
        let config = parse_config_str(
            r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"],
                    "ОтключенныеСценарии": ["ЛокальныйОтключенный.os"]
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            config.scenarios.disabled_scenarios,
            vec!["ЛокальныйОтключенный"]
        );
        assert_eq!(
            config.warnings,
            vec![ConfigWarning {
                message: "unknown disabled scenario id in global: ЛокальныйОтключенный".to_owned()
            }]
        );
    }

    #[test]
    fn config_unknown_top_level_keys_are_warnings_in_zero_x() {
        let config = parse_config_str(
            r#"{
                "GLOBAL": {},
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["УдалениеЛишнихКонцевыхПробелов.os"]
                },
                "EXPERIMENT": true
            }"#,
        )
        .unwrap();

        assert_eq!(
            config.warnings,
            vec![ConfigWarning {
                message: "unknown top-level config key: EXPERIMENT".to_owned()
            }]
        );
    }

    fn temp_repo(test_name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .expect("current dir must be available")
            .join("target")
            .join("config-tests")
            .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
        fs::create_dir_all(&path).expect("temporary config test repo must be created");
        path
    }

    fn write_config(path: &Path, content: &str) {
        fs::write(path, content).expect("test config must be writable");
    }
}
