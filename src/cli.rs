use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

pub use prec_bsl_output::OutputFormat;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    PrekHook(PrekHookArgs),
    ExecRules(ExecRulesArgs),
    Check(CheckArgs),
    Fix(FixArgs),
    Help(HelpTopic),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelpTopic {
    General,
    PrekHook,
    ExecRules,
    Check,
    Fix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrekHookArgs {
    pub config: Option<PathBuf>,
    pub source_dir: Option<PathBuf>,
    pub rules: Option<RuleList>,
    pub format: OutputFormat,
    pub merge_commit_skip: MergeCommitSkipMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeCommitSkipMode {
    None,
    Any,
    Remote,
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
pub struct CheckArgs {
    pub repo: PathBuf,
    pub config: Option<PathBuf>,
    pub source_dirs: Option<SourceDirList>,
    pub rules: Option<RuleList>,
    pub exclude_rules: Option<RuleList>,
    pub format: OutputFormat,
    pub mode: FileSelectionMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixArgs {
    pub repo: PathBuf,
    pub config: Option<PathBuf>,
    pub source_dirs: Option<SourceDirList>,
    pub rules: Option<RuleList>,
    pub exclude_rules: Option<RuleList>,
    pub format: OutputFormat,
    pub mode: FileSelectionMode,
    pub dry_run: bool,
    /// `fix --restage`: restage the fixed working-tree files back into the
    /// Git index, like the `prek-hook` restaging step.
    pub restage: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileSelectionMode {
    #[default]
    All,
    Changed,
    Index,
    Worktree,
}

impl FileSelectionMode {
    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "all" => Ok(Self::All),
            "changed" => Ok(Self::Changed),
            "index" => Ok(Self::Index),
            "worktree" => Ok(Self::Worktree),
            other => Err(CliError::new(format!(
                "недопустимое значение --mode: {other}; ожидается all, changed, index или worktree"
            ))),
        }
    }
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
        Some("check") => parse_check(rest),
        Some("fix") => parse_fix(rest),
        Some(other) => Err(CliError::new(format!("неизвестная команда: {other}"))),
        None => Err(CliError::new("имя команды должно быть UTF-8")),
    }
}

pub fn help(topic: HelpTopic) -> &'static str {
    match topic {
        HelpTopic::General => {
            "Использование: prec-bsl <команда> [параметры]\n\nКоманды:\n  prek-hook    Запустить hook-режим по staged Git index\n  exec-rules   (устарела) Запустить выбранные правила по репозиторию; используйте fix --mode all\n  check        Проверить исходники без изменения файлов\n  fix          Применить фикстеры к исходникам\n\nИспользуйте 'prec-bsl <команда> --help', чтобы посмотреть параметры команды."
        }
        HelpTopic::PrekHook => {
            "Использование: prec-bsl prek-hook [--config <path>] [--source-dir <path>] [--rules <list>] [--format text|json] [--ignore-merge-commit|--ignore-remote-merge-commit]\n\nПараметры:\n  --config <path>                 Путь к v8config.json\n  --source-dir <path>             Переопределение корня исходников\n  --rules <list>                  Список правил через запятую\n  --format text|json              Формат вывода, по умолчанию text\n  --ignore-merge-commit           Успешно выйти без обработки, когда присутствует MERGE_HEAD\n  --ignore-remote-merge-commit    Успешно выйти только когда MERGE_HEAD достижим из refs/remotes/*"
        }
        HelpTopic::ExecRules => {
            "Использование: prec-bsl exec-rules <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--format text|json]\n\nПараметры:\n  --config <path>       Путь к v8config.json\n  --source-dir <list>   Список корней исходников через запятую\n  --rules <list>        Список правил через запятую\n  --format text|json    Формат вывода, по умолчанию text\n\nКоманда устарела: используйте prec-bsl fix --mode all"
        }
        HelpTopic::Check => {
            "Использование: prec-bsl check <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--exclude-rules <list>] [--format text|json] [--mode <m>]\n\nПараметры:\n  --config <path>       Путь к v8config.json\n  --source-dir <list>   Список корней исходников через запятую\n  --rules <list>        Список правил через запятую\n  --exclude-rules <list>    Список правил для исключения через запятую\n  --format text|json    Формат вывода, по умолчанию text\n  --mode <m>            Набор файлов: all (по умолчанию), changed, index, worktree\n\nКоманда только диагностирует нарушения и никогда не изменяет файлы.\nКод выхода: 0 — нарушений нет, 1 — есть нарушения, 2 — ошибка выполнения."
        }
        HelpTopic::Fix => {
            "Использование: prec-bsl fix <repo> [--config <path>] [--source-dir <list>] [--rules <list>] [--exclude-rules <list>] [--format text|json] [--mode <m>] [--dry-run] [--restage]\n\nПараметры:\n  --config <path>       Путь к v8config.json\n  --source-dir <list>   Список корней исходников через запятую\n  --rules <list>        Список правил через запятую\n  --exclude-rules <list>    Список правил для исключения через запятую\n  --format text|json    Формат вывода, по умолчанию text\n  --mode <m>            Набор файлов: all (по умолчанию), changed, index, worktree\n  --dry-run             Показать изменения (дифф), не записывая их в файлы\n  --restage             После применения правок добавить исправленные файлы в Git index (не влияет на dry-run)\n\nКод выхода: 0 — правок не потребовалось, 1 — правки применены (в dry-run: есть что применять), 2 — ошибка."
        }
    }
}

fn parse_prek_hook(args: &[OsString]) -> Result<CliCommand, CliError> {
    let mut parsed = PrekHookArgs {
        config: None,
        source_dir: None,
        rules: None,
        format: OutputFormat::default(),
        merge_commit_skip: MergeCommitSkipMode::None,
    };

    let mut index = 0;
    while index < args.len() {
        let Some(argument) = args[index].to_str() else {
            index += 1;
            continue;
        };

        match argument {
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
            "--ignore-merge-commit" => {
                set_merge_commit_skip(&mut parsed, MergeCommitSkipMode::Any, argument)?;
            }
            "--ignore-remote-merge-commit" => {
                set_merge_commit_skip(&mut parsed, MergeCommitSkipMode::Remote, argument)?;
            }
            other if other.starts_with("--") => {
                return Err(CliError::new(format!(
                    "неизвестный параметр для prek-hook: {other}"
                )));
            }
            _other => {
                // pre-commit-compatible runners may still pass filenames when
                // hook config drifts. Commit mode deliberately ignores them
                // and uses the Git index as the source of truth.
            }
        }
        index += 1;
    }

    Ok(CliCommand::PrekHook(parsed))
}

fn set_merge_commit_skip(
    parsed: &mut PrekHookArgs,
    mode: MergeCommitSkipMode,
    option: &str,
) -> Result<(), CliError> {
    if parsed.merge_commit_skip != MergeCommitSkipMode::None {
        return Err(CliError::new(format!(
            "{option} нельзя использовать вместе с другим параметром пропуска merge commit"
        )));
    }
    parsed.merge_commit_skip = mode;
    Ok(())
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
                    "неизвестный параметр для exec-rules: {other}"
                )));
            }
            _ => {
                if repo.is_some() {
                    return Err(CliError::new(
                        "exec-rules принимает ровно один позиционный аргумент <repo>",
                    ));
                }
                repo = Some(PathBuf::from(&args[index]));
            }
        }
        index += 1;
    }

    let Some(repo) = repo else {
        return Err(CliError::new("не указан обязательный аргумент: <repo>"));
    };

    Ok(CliCommand::ExecRules(ExecRulesArgs {
        repo,
        config,
        source_dirs,
        rules,
        format,
    }))
}

fn parse_check(args: &[OsString]) -> Result<CliCommand, CliError> {
    let mut parsed = CommonCommandArgs::default();
    if let Some(topic) = parse_common_command_args("check", args, &mut parsed)? {
        return Ok(CliCommand::Help(topic));
    }
    require_repo("check", &parsed)?;
    Ok(CliCommand::Check(CheckArgs {
        repo: parsed.repo.expect("repo is required"),
        config: parsed.config,
        source_dirs: parsed.source_dirs,
        rules: parsed.rules,
        exclude_rules: parsed.exclude_rules,
        format: parsed.format,
        mode: parsed.mode,
    }))
}

fn parse_fix(args: &[OsString]) -> Result<CliCommand, CliError> {
    let mut parsed = CommonCommandArgs::default();
    let mut index = 0;
    while index < args.len() {
        let argument = flag(args[index].as_os_str())?;
        if argument == "--dry-run" {
            parsed.dry_run = true;
        } else if argument == "--restage" {
            parsed.restage = true;
        } else if let Some(topic) =
            parse_common_argument("fix", argument, args, &mut index, &mut parsed)?
        {
            return Ok(CliCommand::Help(topic));
        }
        index += 1;
    }
    require_repo("fix", &parsed)?;
    Ok(CliCommand::Fix(FixArgs {
        repo: parsed.repo.expect("repo is required"),
        config: parsed.config,
        source_dirs: parsed.source_dirs,
        rules: parsed.rules,
        exclude_rules: parsed.exclude_rules,
        format: parsed.format,
        mode: parsed.mode,
        dry_run: parsed.dry_run,
        restage: parsed.restage,
    }))
}

#[derive(Default)]
struct CommonCommandArgs {
    repo: Option<PathBuf>,
    config: Option<PathBuf>,
    source_dirs: Option<SourceDirList>,
    rules: Option<RuleList>,
    exclude_rules: Option<RuleList>,
    format: OutputFormat,
    mode: FileSelectionMode,
    dry_run: bool,
    restage: bool,
}

fn parse_common_command_args(
    command: &str,
    args: &[OsString],
    parsed: &mut CommonCommandArgs,
) -> Result<Option<HelpTopic>, CliError> {
    let mut index = 0;
    while index < args.len() {
        let argument = flag(args[index].as_os_str())?;
        if let Some(topic) = parse_common_argument(command, argument, args, &mut index, parsed)? {
            return Ok(Some(topic));
        }
        index += 1;
    }
    Ok(None)
}

fn parse_common_argument(
    command: &str,
    argument: &str,
    args: &[OsString],
    index: &mut usize,
    parsed: &mut CommonCommandArgs,
) -> Result<Option<HelpTopic>, CliError> {
    let help_topic = match command {
        "check" => HelpTopic::Check,
        "fix" => HelpTopic::Fix,
        _ => HelpTopic::General,
    };

    match argument {
        "-h" | "--help" => return Ok(Some(help_topic)),
        "--config" => {
            parsed.config = Some(PathBuf::from(required_value(args, index, "--config")?));
        }
        "--source-dir" => {
            parsed.source_dirs = Some(SourceDirList(required_utf8_value(
                args,
                index,
                "--source-dir",
            )?));
        }
        "--rules" => {
            parsed.rules = Some(RuleList(required_utf8_value(args, index, "--rules")?));
        }
        "--exclude-rules" => {
            parsed.exclude_rules = Some(RuleList(required_utf8_value(
                args,
                index,
                "--exclude-rules",
            )?));
        }
        "--format" => {
            parsed.format = parse_format(&required_utf8_value(args, index, "--format")?)?;
        }
        "--mode" => {
            parsed.mode = FileSelectionMode::parse(&required_utf8_value(args, index, "--mode")?)?;
        }
        other if other.starts_with('-') => {
            return Err(CliError::new(format!(
                "неизвестный параметр для {command}: {other}"
            )));
        }
        _ => {
            if parsed.repo.is_some() {
                return Err(CliError::new(format!(
                    "{command} принимает ровно один позиционный аргумент <repo>"
                )));
            }
            parsed.repo = Some(PathBuf::from(&args[*index]));
        }
    }
    Ok(None)
}

fn require_repo(command: &str, parsed: &CommonCommandArgs) -> Result<(), CliError> {
    if parsed.repo.is_none() {
        return Err(CliError::new(format!(
            "не указан обязательный аргумент: <repo> для {command}"
        )));
    }
    Ok(())
}

fn flag(value: &OsStr) -> Result<&str, CliError> {
    value
        .to_str()
        .ok_or_else(|| CliError::new("имя аргумента должно быть UTF-8"))
}

fn required_value(
    args: &[OsString],
    index: &mut usize,
    option: &str,
) -> Result<OsString, CliError> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| CliError::new(format!("не указано значение для {option}")))
}

fn required_utf8_value(
    args: &[OsString],
    index: &mut usize,
    option: &str,
) -> Result<String, CliError> {
    let value = required_value(args, index, option)?;
    value
        .into_string()
        .map_err(|_| CliError::new(format!("значение {option} должно быть UTF-8")))
}

fn parse_format(value: &str) -> Result<OutputFormat, CliError> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        other => Err(CliError::new(format!(
            "недопустимое значение --format: {other}; ожидается text или json"
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
                merge_commit_skip: MergeCommitSkipMode::None,
            })
        );
    }

    #[test]
    fn help_text_is_human_readable_russian() {
        assert!(help(HelpTopic::General).contains("Использование:"));
        assert!(help(HelpTopic::General).contains("Команды:"));
        assert!(help(HelpTopic::PrekHook).contains("Параметры:"));
        assert!(help(HelpTopic::ExecRules).contains("Формат вывода"));
        assert!(!help(HelpTopic::General).contains("Usage:"));
        assert!(!help(HelpTopic::PrekHook).contains("Options:"));
    }

    #[test]
    fn prek_hook_accepts_ignore_merge_commit() {
        let command = parse_args(["prek-hook", "--ignore-merge-commit"]).unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: None,
                source_dir: None,
                rules: None,
                format: OutputFormat::Text,
                merge_commit_skip: MergeCommitSkipMode::Any,
            })
        );
    }

    #[test]
    fn prek_hook_accepts_ignore_remote_merge_commit() {
        let command = parse_args(["prek-hook", "--ignore-remote-merge-commit"]).unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: None,
                source_dir: None,
                rules: None,
                format: OutputFormat::Text,
                merge_commit_skip: MergeCommitSkipMode::Remote,
            })
        );
    }

    #[test]
    fn prek_hook_rejects_combined_merge_commit_ignore_modes() {
        let error = parse_args([
            "prek-hook",
            "--ignore-merge-commit",
            "--ignore-remote-merge-commit",
        ])
        .unwrap_err();

        assert_eq!(
            error.message(),
            "--ignore-remote-merge-commit нельзя использовать вместе с другим параметром пропуска merge commit"
        );
    }

    #[test]
    fn prek_hook_ignores_positional_filenames_from_runner() {
        let command = parse_args([
            "prek-hook",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "src/ПереданныйФайл.bsl",
            "src/ДругойФайл.bsl",
        ])
        .unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: None,
                source_dir: None,
                rules: Some(RuleList("УдалениеЛишнихКонцевыхПробелов".to_owned())),
                format: OutputFormat::Text,
                merge_commit_skip: MergeCommitSkipMode::None,
            })
        );
    }

    #[test]
    fn prek_hook_ignores_dash_prefixed_positional_filenames_from_runner() {
        let command = parse_args(["prek-hook", "-ПереданныйФайл.bsl"]).unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: None,
                source_dir: None,
                rules: None,
                format: OutputFormat::Text,
                merge_commit_skip: MergeCommitSkipMode::None,
            })
        );
    }

    #[cfg(unix)]
    #[test]
    fn prek_hook_ignores_non_utf8_positional_filenames_from_runner() {
        use std::os::unix::ffi::OsStringExt;

        let command = parse_args(vec![
            OsString::from("prek-hook"),
            OsString::from_vec(b"src/\xff.bsl".to_vec()),
        ])
        .unwrap();

        assert_eq!(
            command,
            CliCommand::PrekHook(PrekHookArgs {
                config: None,
                source_dir: None,
                rules: None,
                format: OutputFormat::Text,
                merge_commit_skip: MergeCommitSkipMode::None,
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
    fn exec_rules_rejects_ignore_merge_commit() {
        let error = parse_args(["exec-rules", ".", "--ignore-merge-commit"]).unwrap_err();

        assert_eq!(
            error.message(),
            "неизвестный параметр для exec-rules: --ignore-merge-commit"
        );
    }

    #[test]
    fn exec_rules_rejects_ignore_remote_merge_commit() {
        let error = parse_args(["exec-rules", ".", "--ignore-remote-merge-commit"]).unwrap_err();

        assert_eq!(
            error.message(),
            "неизвестный параметр для exec-rules: --ignore-remote-merge-commit"
        );
    }

    #[test]
    fn prek_hook_rejects_exclude_rules() {
        let error = parse_args(["prek-hook", "--exclude-rules", "В"]).unwrap_err();

        assert_eq!(
            error.message(),
            "неизвестный параметр для prek-hook: --exclude-rules"
        );
    }

    #[test]
    fn exec_rules_rejects_exclude_rules() {
        let error = parse_args(["exec-rules", ".", "--exclude-rules", "В"]).unwrap_err();

        assert_eq!(
            error.message(),
            "неизвестный параметр для exec-rules: --exclude-rules"
        );
    }

    #[test]
    fn invalid_format_is_clear_error() {
        let error = parse_args(["prek-hook", "--format", "yaml"]).unwrap_err();

        assert_eq!(
            error.message(),
            "недопустимое значение --format: yaml; ожидается text или json"
        );
    }

    #[test]
    fn unknown_command_is_clear_error() {
        let error = parse_args(["unknown"]).unwrap_err();

        assert_eq!(error.message(), "неизвестная команда: unknown");
    }

    #[test]
    fn missing_exec_rules_repo_is_clear_error() {
        let error = parse_args(["exec-rules", "--rules", "Rule1"]).unwrap_err();

        assert_eq!(error.message(), "не указан обязательный аргумент: <repo>");
    }

    #[test]
    fn check_accepts_positional_repo_and_public_options() {
        let command = parse_args([
            "check",
            "/tmp/repo",
            "--config",
            "v8config.json",
            "--source-dir",
            "src,exts/rat",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
            "--format",
            "json",
            "--mode",
            "changed",
        ])
        .unwrap();

        assert_eq!(
            command,
            CliCommand::Check(CheckArgs {
                repo: PathBuf::from("/tmp/repo"),
                config: Some(PathBuf::from("v8config.json")),
                source_dirs: Some(SourceDirList("src,exts/rat".to_owned())),
                rules: Some(RuleList("УдалениеЛишнихКонцевыхПробелов".to_owned())),
                exclude_rules: None,
                format: OutputFormat::Json,
                mode: FileSelectionMode::Changed,
            })
        );
    }

    #[test]
    fn check_defaults_to_all_mode_and_text_format() {
        let command = parse_args(["check", "."]).unwrap();

        assert_eq!(
            command,
            CliCommand::Check(CheckArgs {
                repo: PathBuf::from("."),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: None,
                format: OutputFormat::Text,
                mode: FileSelectionMode::All,
            })
        );
    }

    #[test]
    fn check_parses_exclude_rules() {
        let command = parse_args(["check", "repo", "--exclude-rules", "В"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Check(CheckArgs {
                repo: PathBuf::from("repo"),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: Some(RuleList("В".to_owned())),
                format: OutputFormat::Text,
                mode: FileSelectionMode::All,
            })
        );
    }

    #[test]
    fn fix_parses_exclude_rules() {
        let command = parse_args(["fix", "repo", "--exclude-rules", "В"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Fix(FixArgs {
                repo: PathBuf::from("repo"),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: Some(RuleList("В".to_owned())),
                format: OutputFormat::Text,
                mode: FileSelectionMode::All,
                dry_run: false,
                restage: false,
            })
        );
    }

    #[test]
    fn check_exclude_rules_requires_value() {
        let error = parse_args(["check", ".", "--exclude-rules"]).unwrap_err();

        assert_eq!(error.message(), "не указано значение для --exclude-rules");
    }

    #[test]
    fn check_repeated_exclude_rules_last_wins() {
        let command =
            parse_args(["check", ".", "--exclude-rules", "А", "--exclude-rules", "В"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Check(CheckArgs {
                repo: PathBuf::from("."),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: Some(RuleList("В".to_owned())),
                format: OutputFormat::Text,
                mode: FileSelectionMode::All,
            })
        );
    }

    #[test]
    fn check_and_fix_help_mention_exclude_rules() {
        assert!(help(HelpTopic::Check).contains("--exclude-rules"));
        assert!(help(HelpTopic::Fix).contains("--exclude-rules"));
    }

    #[test]
    fn fix_accepts_dry_run_and_mode() {
        let command = parse_args(["fix", "/tmp/repo", "--mode", "worktree", "--dry-run"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Fix(FixArgs {
                repo: PathBuf::from("/tmp/repo"),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: None,
                format: OutputFormat::Text,
                mode: FileSelectionMode::Worktree,
                dry_run: true,
                restage: false,
            })
        );
    }

    #[test]
    fn fix_defaults_to_apply_without_dry_run() {
        let command = parse_args(["fix", ".", "--mode", "index"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Fix(FixArgs {
                repo: PathBuf::from("."),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: None,
                format: OutputFormat::Text,
                mode: FileSelectionMode::Index,
                dry_run: false,
                restage: false,
            })
        );
    }

    #[test]
    fn fix_accepts_restage_flag() {
        let command = parse_args(["fix", "/tmp/repo", "--mode", "index", "--restage"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Fix(FixArgs {
                repo: PathBuf::from("/tmp/repo"),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: None,
                format: OutputFormat::Text,
                mode: FileSelectionMode::Index,
                dry_run: false,
                restage: true,
            })
        );
    }

    #[test]
    fn fix_restage_with_dry_run_is_valid_parse() {
        let command = parse_args(["fix", ".", "--dry-run", "--restage"]).unwrap();

        assert_eq!(
            command,
            CliCommand::Fix(FixArgs {
                repo: PathBuf::from("."),
                config: None,
                source_dirs: None,
                rules: None,
                exclude_rules: None,
                format: OutputFormat::Text,
                mode: FileSelectionMode::All,
                dry_run: true,
                restage: true,
            })
        );
    }

    #[test]
    fn check_rejects_restage_flag() {
        let error = parse_args(["check", ".", "--restage"]).unwrap_err();

        assert_eq!(error.message(), "неизвестный параметр для check: --restage");
    }

    #[test]
    fn invalid_mode_is_clear_error() {
        let error = parse_args(["check", ".", "--mode", "everything"]).unwrap_err();

        assert_eq!(
            error.message(),
            "недопустимое значение --mode: everything; ожидается all, changed, index или worktree"
        );
    }

    #[test]
    fn check_rejects_dry_run_flag() {
        let error = parse_args(["check", ".", "--dry-run"]).unwrap_err();

        assert_eq!(error.message(), "неизвестный параметр для check: --dry-run");
    }

    #[test]
    fn fix_rejects_merge_commit_flags() {
        let error = parse_args(["fix", ".", "--ignore-merge-commit"]).unwrap_err();

        assert_eq!(
            error.message(),
            "неизвестный параметр для fix: --ignore-merge-commit"
        );
    }

    #[test]
    fn missing_check_repo_is_clear_error() {
        let error = parse_args(["check", "--rules", "Rule1"]).unwrap_err();

        assert_eq!(
            error.message(),
            "не указан обязательный аргумент: <repo> для check"
        );
    }

    #[test]
    fn missing_fix_repo_is_clear_error() {
        let error = parse_args(["fix", "--mode", "all"]).unwrap_err();

        assert_eq!(
            error.message(),
            "не указан обязательный аргумент: <repo> для fix"
        );
    }

    #[test]
    fn check_help_mentions_exit_codes() {
        assert!(help(HelpTopic::Check).contains("Код выхода"));
        assert!(help(HelpTopic::Fix).contains("--dry-run"));
        assert!(help(HelpTopic::Fix).contains("--restage"));
        assert!(help(HelpTopic::General).contains("check"));
        assert!(help(HelpTopic::General).contains("fix"));
        assert!(help(HelpTopic::ExecRules).contains("устарела"));
    }
}
