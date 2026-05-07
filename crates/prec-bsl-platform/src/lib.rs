use std::env;
use std::path::{Path, PathBuf};

use serde_json::Value;

use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::SourceFileKind;

pub const EXTERNAL_ARTIFACTS_RULE: &str = "РазборОтчетовОбработокРасширений";
pub const EXTERNAL_ARTIFACTS_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    EXTERNAL_ARTIFACTS_RULE,
    "РазборОтчетовОбработокРасширений.os",
    external_artifacts,
);

const PLATFORM_EXECUTABLE_CANDIDATES: &[&str] = &["1cv8", "1cv8c", "1cv8.exe", "1cv8c.exe"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalArtifactSettings {
    pub platform_version: Option<String>,
}

impl ExternalArtifactSettings {
    pub fn from_settings(settings: Option<&Value>) -> Result<Self, String> {
        let Some(settings) = settings else {
            return Ok(Self {
                platform_version: None,
            });
        };
        let Value::Object(settings) = settings else {
            return Err("scenario settings must be a JSON object".to_owned());
        };

        let use_defaults = settings
            .get("ИспользоватьНастройкиПоУмолчанию")
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| "ИспользоватьНастройкиПоУмолчанию must be a boolean".to_owned())
            })
            .transpose()?
            .unwrap_or(true);
        let platform_version = settings
            .get("ВерсияПлатформы")
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .map(str::to_owned)
                    .ok_or_else(|| "ВерсияПлатформы must be a string".to_owned())
            })
            .transpose()?
            .filter(|value| !value.is_empty());

        Ok(Self {
            platform_version: if use_defaults { None } else { platform_version },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalArtifactBoundary {
    Skipped(String),
    Failed(String),
}

pub fn external_artifacts(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::ExternalArtifact {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only external report, processing, and extension artifacts",
        ));
    }

    let settings = match ExternalArtifactSettings::from_settings(context.settings) {
        Ok(settings) => settings,
        Err(message) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
    };
    let discovered_platform = discover_platform_executable(settings.platform_version.as_deref());
    let result = evaluate_external_artifact_boundary(
        &context.file.repo_path,
        context.file.kind,
        &settings,
        discovered_platform.as_deref(),
    );

    ScenarioRun::single(match result {
        ExternalArtifactBoundary::Skipped(message) => {
            ScenarioResult::skipped(context.rule_id, context.file.repo_path.clone(), message)
        }
        ExternalArtifactBoundary::Failed(message) => {
            ScenarioResult::hard_failure(context.rule_id, context.file.repo_path.clone(), message)
        }
    })
}

pub fn evaluate_external_artifact_boundary(
    path: &Path,
    kind: SourceFileKind,
    settings: &ExternalArtifactSettings,
    discovered_platform: Option<&Path>,
) -> ExternalArtifactBoundary {
    if kind != SourceFileKind::ExternalArtifact {
        return ExternalArtifactBoundary::Skipped(
            "scenario handles only external report, processing, and extension artifacts".to_owned(),
        );
    }

    let artifact_kind = match external_artifact_kind(path) {
        Some(artifact_kind) => artifact_kind,
        None => {
            return ExternalArtifactBoundary::Skipped(
                "scenario handles only .epf, .erf, and .cfe artifacts".to_owned(),
            );
        }
    };

    let Some(platform) = discovered_platform else {
        let version = settings
            .platform_version
            .as_deref()
            .map(|version| format!("; required version path fragment: {version}"))
            .unwrap_or_default();
        return ExternalArtifactBoundary::Failed(format!(
            "1C platform executable is required to unpack {artifact_kind}; searched PATH for {}{version}",
            PLATFORM_EXECUTABLE_CANDIDATES.join(", ")
        ));
    };

    let command_contract = match artifact_kind {
        "external report/processing" => "/DumpExternalDataProcessorOrReportToFiles",
        "extension" => "/LoadCfg followed by /DumpConfigToFiles -Extension",
        _ => unreachable!("artifact kind is controlled by extension matching"),
    };
    ExternalArtifactBoundary::Failed(format!(
        "1C platform executable discovered at {}; {command_contract} execution is outside the T30 boundary and was not run",
        platform.display()
    ))
}

pub fn discover_platform_executable(required_version: Option<&str>) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    discover_platform_executable_in_paths(env::split_paths(&path), required_version)
}

pub fn discover_platform_executable_in_paths<I, P>(
    paths: I,
    required_version: Option<&str>,
) -> Option<PathBuf>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let required_version = required_version
        .map(str::trim)
        .filter(|value| !value.is_empty());
    for directory in paths {
        let directory = directory.as_ref();
        for executable_name in PLATFORM_EXECUTABLE_CANDIDATES {
            let candidate = directory.join(executable_name);
            if !candidate.is_file() || !is_executable(&candidate) {
                continue;
            }
            if required_version.is_some_and(|version| {
                !candidate.to_string_lossy().contains(version)
                    && !directory.to_string_lossy().contains(version)
            }) {
                continue;
            }
            return Some(candidate);
        }
    }

    None
}

fn external_artifact_kind(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("epf") => {
            Some("external report/processing")
        }
        Some(extension) if extension.eq_ignore_ascii_case("erf") => {
            Some("external report/processing")
        }
        Some(extension) if extension.eq_ignore_ascii_case("cfe") => Some("extension"),
        _ => None,
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}
