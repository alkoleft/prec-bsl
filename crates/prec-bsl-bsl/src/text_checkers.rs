use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use prec_bsl_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use prec_bsl_source::SourceFileKind;

pub const PROFANITY_RULE: &str = "ПроверкаНецензурныхСлов";

const DICTIONARY_SETTING: &str = "ФайлСНецензурнымиСловами";
const DEFAULT_DICTIONARY: &str = "НецензурныеСлова.txt";

pub fn profanity(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::BslModule {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only BSL modules",
        ));
    }

    let dictionary = match load_dictionary(context) {
        DictionaryLoad::Loaded(dictionary) => dictionary,
        DictionaryLoad::Skipped(message) => {
            return ScenarioRun::single(ScenarioResult::skipped(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
        DictionaryLoad::Failed(message) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
    };

    let path = context.repo_root.join(&context.file.repo_path);
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to read file: {error}"),
            ));
        }
    };

    let results = find_profanity_matches(&input, &dictionary)
        .into_iter()
        .map(|matched| {
            ScenarioResult::warning(
                context.rule_id,
                context.file.repo_path.clone(),
                format!(
                    "matched dictionary word '{}' at line {}",
                    matched.word, matched.line
                ),
            )
        })
        .collect();

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DictionaryLoad {
    Loaded(Vec<String>),
    Skipped(String),
    Failed(String),
}

fn load_dictionary(context: &ScenarioExecutionContext<'_>) -> DictionaryLoad {
    let dictionary_path = match dictionary_path(context.settings) {
        Ok(Some(path)) => path,
        Ok(None) => {
            let path = PathBuf::from(DEFAULT_DICTIONARY);
            return load_default_dictionary(context.repo_root, &path);
        }
        Err(message) => return DictionaryLoad::Failed(message),
    };

    if !is_repository_relative_path(&dictionary_path) {
        return DictionaryLoad::Failed(format!(
            "dictionary path must be repository-relative: {}",
            dictionary_path.display()
        ));
    }

    load_configured_dictionary(context.repo_root, &dictionary_path)
}

fn dictionary_path(settings: Option<&Value>) -> Result<Option<PathBuf>, String> {
    let Some(settings) = settings else {
        return Ok(None);
    };
    let Some(value) = settings.get(DICTIONARY_SETTING) else {
        return Ok(None);
    };
    match value {
        Value::String(path) if path.trim().is_empty() => Err(format!(
            "dictionary setting {DICTIONARY_SETTING} must not be empty"
        )),
        Value::String(path) => Ok(Some(PathBuf::from(path.trim()))),
        _ => Err(format!(
            "dictionary setting {DICTIONARY_SETTING} must be a string"
        )),
    }
}

fn load_default_dictionary(repo_root: &Path, path: &Path) -> DictionaryLoad {
    if !repo_root.join(path).is_file() {
        return DictionaryLoad::Skipped(format!(
            "profanity dictionary is not configured or found: {}",
            path.display()
        ));
    }

    match fs::read_to_string(repo_root.join(path)) {
        Ok(content) => DictionaryLoad::Loaded(parse_dictionary(&content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            DictionaryLoad::Skipped(format!(
                "profanity dictionary is not configured or found: {}",
                path.display()
            ))
        }
        Err(error) => DictionaryLoad::Failed(format!(
            "failed to read profanity dictionary {}: {error}",
            path.display()
        )),
    }
}

fn load_configured_dictionary(repo_root: &Path, path: &Path) -> DictionaryLoad {
    match fs::read_to_string(repo_root.join(path)) {
        Ok(content) => DictionaryLoad::Loaded(parse_dictionary(&content)),
        Err(error) => DictionaryLoad::Failed(format!(
            "failed to read configured profanity dictionary {}: {error}",
            path.display()
        )),
    }
}

fn parse_dictionary(input: &str) -> Vec<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|word| !word.is_empty() && !word.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfanityMatch {
    pub line: usize,
    pub word: String,
}

pub fn find_profanity_matches(input: &str, dictionary: &[String]) -> Vec<ProfanityMatch> {
    input
        .lines()
        .enumerate()
        .flat_map(|(line_index, line)| {
            let line_lowercase = line.to_lowercase();
            dictionary
                .iter()
                .filter(move |word| line_lowercase.contains(&word.to_lowercase()))
                .map(move |word| ProfanityMatch {
                    line: line_index + 1,
                    word: word.clone(),
                })
        })
        .collect()
}

fn is_repository_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                Component::Normal(_) | Component::CurDir | Component::ParentDir
            )
        })
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profanity_matches_dictionary_words_case_insensitively() {
        let dictionary = vec!["плохоеСлово".to_owned(), "BAD".to_owned()];

        assert_eq!(
            find_profanity_matches("строка\nТут плохоеслово\nbad token", &dictionary),
            vec![
                ProfanityMatch {
                    line: 2,
                    word: "плохоеСлово".to_owned()
                },
                ProfanityMatch {
                    line: 3,
                    word: "BAD".to_owned()
                }
            ]
        );
    }

    #[test]
    fn dictionary_ignores_empty_lines_and_comments() {
        assert_eq!(
            parse_dictionary("  \n# comment\n слово \r\n"),
            vec!["слово".to_owned()]
        );
    }
}
