use serde_json::Value;

use crate::error::ConfigError;
use crate::model::{ConfigWarning, ResolvedConfig};
use crate::path::is_repository_relative_path;
use crate::scenario::{ScenarioCatalog, ScenarioSupport, normalize_scenario_id};

pub(crate) fn validate_config(
    config: &ResolvedConfig,
    catalog: ScenarioCatalog<'_>,
) -> Result<(), ConfigError> {
    let mut errors = Vec::new();

    validate_enabled_scenarios(
        &config.scenarios.global_scenarios,
        "global",
        catalog,
        &mut errors,
    );
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
            validate_enabled_scenarios(global_scenarios, project_path, catalog, &mut errors);
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

pub(crate) fn credential_key_paths(value: &Value) -> Vec<String> {
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

pub(crate) fn add_validation_warnings(config: &mut ResolvedConfig, catalog: ScenarioCatalog<'_>) {
    add_unknown_disabled_warnings(
        &config.scenarios.disabled_scenarios,
        "global",
        catalog,
        &mut config.warnings,
    );
    for (project_path, project) in &config.scenarios.projects {
        if let Some(disabled_scenarios) = &project.disabled_scenarios {
            add_unknown_disabled_warnings(
                disabled_scenarios,
                project_path,
                catalog,
                &mut config.warnings,
            );
        }
    }
}

fn add_unknown_disabled_warnings(
    scenarios: &[String],
    scope: &str,
    catalog: ScenarioCatalog<'_>,
    warnings: &mut Vec<ConfigWarning>,
) {
    for scenario in scenarios {
        if catalog.find(scenario).is_none() {
            warnings.push(ConfigWarning {
                message: format!(
                    "unknown disabled scenario id in {scope}: {}",
                    normalize_scenario_id(scenario)
                ),
            });
        }
    }
}

fn validate_enabled_scenarios(
    scenarios: &[String],
    scope: &str,
    catalog: ScenarioCatalog<'_>,
    errors: &mut Vec<String>,
) {
    for scenario in scenarios {
        let normalized = normalize_scenario_id(scenario);
        match catalog.find(scenario).map(|definition| definition.support) {
            Some(ScenarioSupport::RequiredV1 | ScenarioSupport::Compatibility) => {}
            Some(ScenarioSupport::Unsupported) => errors.push(format!(
                "unsupported built-in scenario in v1 enabled in {scope}: {normalized}"
            )),
            None => errors.push(format!(
                "unsupported repository-local scenario in v1 enabled in {scope}: {normalized}; dynamic local .os execution is not supported in v1"
            )),
        }
    }
}
