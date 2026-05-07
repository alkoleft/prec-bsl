use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::*;
use crate::resolve::CONFIG_FILE_NAME;
use crate::{ScenarioCatalog, ScenarioMetadata, UNSUPPORTED_ORDINARY_FORMS};

const TEST_SCENARIOS: &[ScenarioMetadata] = &[
    ScenarioMetadata::required_v1(
        "УдалениеЛишнихКонцевыхПробелов",
        "УдалениеЛишнихКонцевыхПробелов.os",
    ),
    ScenarioMetadata::required_v1("УдалениеЛишнихПустыхСтрок", "УдалениеЛишнихПустыхСтрок.os"),
    ScenarioMetadata::required_v1("ПроверкаНецензурныхСлов", "ПроверкаНецензурныхСлов.os"),
    ScenarioMetadata::required_v1(
        "ПроверкаДублейПроцедурИФункций",
        "ПроверкаДублейПроцедурИФункций.os",
    ),
    ScenarioMetadata::required_v1("СортировкаСостава", "СортировкаСостава.os"),
    ScenarioMetadata::compatibility("СортировкаДереваМетаданных", "СортировкаСостава.os"),
    ScenarioMetadata::compatibility("СортировкаСоставаПодсистем", "СортировкаСостава.os"),
    ScenarioMetadata::unsupported(
        UNSUPPORTED_ORDINARY_FORMS,
        "РазборОбычныхФормНаИсходники.os",
    ),
];

fn test_catalog() -> ScenarioCatalog<'static> {
    ScenarioCatalog::new(TEST_SCENARIOS)
}

fn parse_config_str(source: &str) -> Result<ResolvedConfig, ConfigError> {
    parse_config_str_with_catalog(source, test_catalog())
}

fn resolve_config(request: &ConfigResolveRequest) -> Result<ResolvedConfig, ConfigError> {
    resolve_config_with_catalog(request, test_catalog())
}

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
    request.rule_override = Some("УдалениеЛишнихПустыхСтрок.os,ПроверкаНецензурныхСлов".to_owned());

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
        &["unsupported built-in scenario in v1 enabled in global: РазборОбычныхФормНаИсходники"]
    );
}

#[test]
fn config_enabled_compatibility_sorting_scenarios_are_supported() {
    let config = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": [
                        "СортировкаДереваМетаданных.os",
                        "СортировкаСоставаПодсистем"
                    ],
                    "Проекты": {
                        "configuration": {
                            "ГлобальныеСценарии": [
                                "СортировкаСоставаПодсистем.os"
                            ]
                        }
                    }
                }
            }"#,
    )
    .unwrap();

    assert_eq!(
        config.scenarios.enabled_scenarios(),
        vec!["СортировкаДереваМетаданных", "СортировкаСоставаПодсистем"]
    );
    assert_eq!(
        config.enabled_scenarios_for_path(Path::new("configuration/src/Subsystems/Демо/Демо.mdo")),
        vec!["СортировкаСоставаПодсистем"]
    );
}

#[test]
fn config_enabled_repository_local_scenario_fails_validation() {
    let error = parse_config_str(
        r#"{
                "Precommt4onecСценарии": {
                    "ГлобальныеСценарии": ["ДобавлениеТестовВРасширение.os"]
                }
            }"#,
    )
    .unwrap_err();

    assert_eq!(
        error.validation_messages(),
        &[
            "unsupported repository-local scenario in v1 enabled in global: ДобавлениеТестовВРасширение; dynamic local .os execution is not supported in v1"
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
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join("config-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary config test repo must be created");
    path
}

fn write_config(path: &Path, content: &str) {
    fs::write(path, content).expect("test config must be writable");
}
