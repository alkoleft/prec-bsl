use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub use crate::output::OutputFormat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    PrekHook(PrekHookArgs),
    ExecRules(ExecRulesArgs),
    Help(HelpTopic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpTopic {
    General,
    PrekHook,
    ExecRules,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekHookArgs {
    pub config: Option<PathBuf>,
    pub source_dir: Option<PathBuf>,
    pub rules: Option<RuleList>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecRulesArgs {
    pub repo: PathBuf,
    pub config: Option<PathBuf>,
    pub source_dirs: Option<SourceDirList>,
    pub rules: Option<RuleList>,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleList(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDirList(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    message: String,
}

impl CliError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub fn parse_env() -> Result<CliCommand, CliError> {
    parse_args(std::env::args_os().skip(1))
}

pub fn parse_args<I, S>(args: I) -> Result<CliCommand, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();

    let Some((command, rest)) = args.split_first() else {
        return Ok(CliCommand::Help(HelpTopic::General));
    };

    match command.to_str() {
        Some("-h" | "--help") => Ok(CliCommand::Help(HelpTopic::General)),
        Some("prek-hook") => parse_prek_hook(rest),
        Some("exec-rules") => parse_exec_rules(rest),
        Some(other) => Err(CliError::new(format!("unknown command: {other}"))),
        None => Err(CliError::new("command name must be valid UTF-8")),
    }
}

pub fn help(topic: HelpTopic) -> &'static str {
    match topic {
        HelpTopic::General => {
            "Usage: prec-bsl <command> [options]\n\nCommands:\n  prek-hook    Run commit hook mode over the staged Git index\n  exec-rules   Run selected rules over a repository\n\nUse 'prec-bsl <command> --help' for command options."
        }
        HelpTopic::PrekHook => {
            "Usage: prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json]\n\nOptions:\n  --config <path>       Path to v8config.json\n  --source-dir <path>   Source root override\n  --rules <list>        Comma-separated rule list\n  --format text|json    Output format, defaults to text"
        }
        HelpTopic::ExecRules => {
            "Usage: prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]\n\nOptions:\n  --config <path>       Path to v8config.json\n  --source-dir <list>   Comma-separated source root list\n  --rules <list>        Comma-separated rule list\n  --format text|json    Output format, defaults to text"
        }
    }
}

fn parse_prek_hook(args: &[OsString]) -> Result<CliCommand, CliError> {
    let mut parsed = PrekHookArgs {
        config: None,
        source_dir: None,
        rules: None,
        format: OutputFormat::default(),
    };

    let mut index = 0;
    while index < args.len() {
        match flag(args[index].as_os_str())? {
            "-h" | "--help" => return Ok(CliCommand::Help(HelpTopic::PrekHook)),
            "--config" => {
                parsed.config = Some(PathBuf::from(required_value(args, &mut index, "--config")?));
            }
            "--source-dir" => {
                parsed.source_dir = Some(PathBuf::from(required_value(
                    args,
                    &mut index,
                    "--source-dir",
                )?));
            }
            "--rules" => {
                parsed.rules = Some(RuleList(required_utf8_value(args, &mut index, "--rules")?));
            }
            "--format" => {
                parsed.format = parse_format(&required_utf8_value(args, &mut index, "--format")?)?;
            }
            other if other.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown option for prek-hook: {other}"
                )));
            }
            other => {
                return Err(CliError::new(format!(
                    "unexpected positional argument for prek-hook: {other}"
                )));
            }
        }
        index += 1;
    }

    Ok(CliCommand::PrekHook(parsed))
}

fn parse_exec_rules(args: &[OsString]) -> Result<CliCommand, CliError> {
    let mut repo = None;
    let mut config = None;
    let mut source_dirs = None;
    let mut rules = None;
    let mut format = OutputFormat::default();

    let mut index = 0;
    while index < args.len() {
        match flag(args[index].as_os_str())? {
            "-h" | "--help" => return Ok(CliCommand::Help(HelpTopic::ExecRules)),
            "--config" => {
                config = Some(PathBuf::from(required_value(args, &mut index, "--config")?));
            }
            "--source-dir" => {
                source_dirs = Some(SourceDirList(required_utf8_value(
                    args,
                    &mut index,
                    "--source-dir",
                )?));
            }
            "--rules" => {
                rules = Some(RuleList(required_utf8_value(args, &mut index, "--rules")?));
            }
            "--format" => {
                format = parse_format(&required_utf8_value(args, &mut index, "--format")?)?;
            }
            other if other.starts_with('-') => {
                return Err(CliError::new(format!(
                    "unknown option for exec-rules: {other}"
                )));
            }
            _ => {
                if repo.is_some() {
                    return Err(CliError::new(
                        "exec-rules accepts exactly one positional <repo>",
                    ));
                }
                repo = Some(PathBuf::from(&args[index]));
            }
        }
        index += 1;
    }

    let Some(repo) = repo else {
        return Err(CliError::new("missing required argument: <repo>"));
    };

    Ok(CliCommand::ExecRules(ExecRulesArgs {
        repo,
        config,
        source_dirs,
        rules,
        format,
    }))
}

fn flag(value: &OsStr) -> Result<&str, CliError> {
    value
        .to_str()
        .ok_or_else(|| CliError::new("argument name must be valid UTF-8"))
}

fn required_value(
    args: &[OsString],
    index: &mut usize,
    option: &str,
) -> Result<OsString, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| CliError::new(format!("missing value for {option}")))
}

fn required_utf8_value(
    args: &[OsString],
    index: &mut usize,
    option: &str,
) -> Result<String, CliError> {
    let value = required_value(args, index, option)?;
    value
        .into_string()
        .map_err(|_| CliError::new(format!("value for {option} must be valid UTF-8")))
}

fn parse_format(value: &str) -> Result<OutputFormat, CliError> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => Err(CliError::new(format!(
            "invalid value for --format: {other}; expected text or json"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prek_hook_accepts_public_options() {
        let command = parse_args([
            "prek-hook",
            "--config",
            "v8config.json",
            "--source-dir",
            "src/cf",
            "--rules",
            "Rule1,Rule2",
            "--format",
            "json",
        ])
        .unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: Some(PathBuf::from("v8config.json")),
                source_dir: Some(PathBuf::from("src/cf")),
                rules: Some(RuleList("Rule1,Rule2".to_owned())),
                format: OutputFormat::Json,
            })
        );
    }

    #[test]
    fn exec_rules_accepts_positional_repo() {
        let command = parse_args([
            "exec-rules",
            "/tmp/repo",
            "--config",
            "custom-v8config.json",
            "--source-dir",
            "configuration,exts/rat",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--format",
            "text",
        ])
        .unwrap();

        assert_eq!(
            command,
            CliCommand::ExecRules(ExecRulesArgs {
                repo: PathBuf::from("/tmp/repo"),
                config: Some(PathBuf::from("custom-v8config.json")),
                source_dirs: Some(SourceDirList("configuration,exts/rat".to_owned())),
                rules: Some(RuleList("УдалениеЛишнихКонцевыхПробелов".to_owned())),
                format: OutputFormat::Text,
            })
        );
    }

    #[test]
    fn exec_rules_accepts_options_before_repo() {
        let command = parse_args(["exec-rules", "--format", "json", "."]).unwrap();

        assert_eq!(
            command,
            CliCommand::ExecRules(ExecRulesArgs {
                repo: PathBuf::from("."),
                config: None,
                source_dirs: None,
                rules: None,
                format: OutputFormat::Json,
            })
        );
    }

    #[test]
    fn invalid_format_is_clear_error() {
        let error = parse_args(["prek-hook", "--format", "yaml"]).unwrap_err();

        assert_eq!(
            error.message(),
            "invalid value for --format: yaml; expected text or json"
        );
    }

    #[test]
    fn unknown_command_is_clear_error() {
        let error = parse_args(["unknown"]).unwrap_err();

        assert_eq!(error.message(), "unknown command: unknown");
    }

    #[test]
    fn missing_exec_rules_repo_is_clear_error() {
        let error = parse_args(["exec-rules", "--rules", "Rule1"]).unwrap_err();

        assert_eq!(error.message(), "missing required argument: <repo>");
    }
}
