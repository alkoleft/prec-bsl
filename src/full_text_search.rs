use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde_json::Value;

use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use crate::source_files::SourceFileKind;
use crate::xml_edt::{parse_document, write_validated_xml};

pub const DISABLE_FULL_TEXT_SEARCH_RULE: &str = "ОтключениеПолнотекстовогоПоиска";

const EXCLUDED_METADATA_SETTING: &str = "МетаданныеДляИсключения";
const USE_VALUE: &str = "Use";
const DONT_USE_VALUE: &str = "DontUse";

pub fn disable_full_text_search(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if !is_metadata_file(context.file.kind) {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only XML/EDT metadata files",
        ));
    }

    let exclusions =
        match FullTextSearchExclusions::from_settings(context.settings, &context.file.repo_path) {
            Ok(exclusions) => exclusions,
            Err(message) => {
                return ScenarioRun::single(ScenarioResult::hard_failure(
                    context.rule_id,
                    context.file.repo_path.clone(),
                    message,
                ));
            }
        };

    if exclusions.skip_file() {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "metadata file is excluded from full-text search disabling",
        ));
    }

    let absolute_path = context.repo_root.join(&context.file.repo_path);
    let input = match fs::read_to_string(&absolute_path) {
        Ok(input) => input,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to read file: {error}"),
            ));
        }
    };

    match disable_full_text_search_text(
        &context.file.repo_path,
        context.file.kind,
        &input,
        &exclusions,
    ) {
        FullTextSearchDisabling::Clean => ScenarioRun::clean(),
        FullTextSearchDisabling::Modified(output) => {
            if let Err(error) = fs::write(&absolute_path, output) {
                return ScenarioRun::single(ScenarioResult::hard_failure(
                    context.rule_id,
                    context.file.repo_path.clone(),
                    format!("failed to write file: {error}"),
                ));
            }
            ScenarioRun::single(ScenarioResult::modified(
                context.rule_id,
                context.file.repo_path.clone(),
                "disabled full-text search metadata settings",
            ))
        }
        FullTextSearchDisabling::Failed(message) => ScenarioRun::single(
            ScenarioResult::hard_failure(context.rule_id, context.file.repo_path.clone(), message),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullTextSearchDisabling {
    Clean,
    Modified(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FullTextSearchExclusions {
    DisableAll,
    SkipFile,
    PreserveAttributes(BTreeSet<String>),
}

impl FullTextSearchExclusions {
    pub fn from_settings(settings: Option<&Value>, repo_path: &Path) -> Result<Self, String> {
        let Some(settings) = settings else {
            return Ok(Self::DisableAll);
        };
        let Some(value) = settings.get(EXCLUDED_METADATA_SETTING) else {
            return Ok(Self::DisableAll);
        };
        let object = value.as_object().ok_or_else(|| {
            format!("{EXCLUDED_METADATA_SETTING} must be an object mapping metadata paths to string arrays")
        })?;
        if object.is_empty() {
            return Ok(Self::DisableAll);
        }

        let normalized_repo_path = normalize_metadata_path(repo_path);
        let Some((_path, value)) = object.iter().find(|(path, _value)| {
            normalize_metadata_path(Path::new(path)) == normalized_repo_path
        }) else {
            return Ok(Self::DisableAll);
        };

        let values = value.as_array().ok_or_else(|| {
            format!("{EXCLUDED_METADATA_SETTING} entry for {normalized_repo_path} must be a string array")
        })?;
        if values.is_empty() {
            return Ok(Self::SkipFile);
        }

        let mut names = BTreeSet::new();
        for value in values {
            let Some(name) = value.as_str() else {
                return Err(format!(
                    "{EXCLUDED_METADATA_SETTING} entry for {normalized_repo_path} must contain only strings"
                ));
            };
            let name = name.trim();
            if name.is_empty() {
                return Err(format!(
                    "{EXCLUDED_METADATA_SETTING} entry for {normalized_repo_path} must not contain empty attribute names"
                ));
            }
            names.insert(name.to_owned());
        }

        Ok(Self::PreserveAttributes(names))
    }

    fn skip_file(&self) -> bool {
        matches!(self, Self::SkipFile)
    }

    fn preserves(&self, owner_names: &[OwnerName]) -> bool {
        match self {
            Self::DisableAll | Self::SkipFile => false,
            Self::PreserveAttributes(names) => {
                let Some(attribute_name) = owner_names.last() else {
                    return false;
                };
                if !is_metadata_attribute_context(&attribute_name.element_name) {
                    return false;
                }
                names.contains(&attribute_name.value)
                    || owner_names.len() >= 2
                        && is_tabular_section_context(
                            &owner_names[owner_names.len() - 2].element_name,
                        )
                        && names.contains(&format!(
                            "{}.{}",
                            owner_names[owner_names.len() - 2].value,
                            attribute_name.value
                        ))
            }
        }
    }
}

pub fn disable_full_text_search_text(
    repo_path: &Path,
    kind: SourceFileKind,
    input: &str,
    exclusions: &FullTextSearchExclusions,
) -> FullTextSearchDisabling {
    if let Err(error) = parse_document(repo_path, kind, input) {
        return FullTextSearchDisabling::Failed(error.to_string());
    }

    let replacements = match collect_replacements(input, exclusions) {
        Ok(replacements) => replacements,
        Err(message) => return FullTextSearchDisabling::Failed(message),
    };
    if replacements.is_empty() {
        return FullTextSearchDisabling::Clean;
    }

    let output = replace_spans(input, &replacements);
    if let Err(error) = write_validated_xml(repo_path, &output) {
        return FullTextSearchDisabling::Failed(error.to_string());
    }

    FullTextSearchDisabling::Modified(output)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElementContext {
    local_name: String,
    object_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerName {
    element_name: String,
    value: String,
}

fn collect_replacements(
    input: &str,
    exclusions: &FullTextSearchExclusions,
) -> Result<Vec<TextSpan>, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<ElementContext>::new();
    let mut replacements = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                stack.push(element_context(&event));
            }
            Ok(Event::Empty(_event)) => {}
            Ok(Event::End(_event)) => {
                stack.pop();
            }
            Ok(Event::Text(event)) => {
                let Some(current) = stack.last() else {
                    continue;
                };
                let text = event
                    .decode()
                    .map_err(|error| format!("failed to decode XML metadata text: {error}"))?
                    .into_owned();

                if is_name_element(&current.local_name) {
                    if stack.len() >= 2 {
                        let parent_index = stack.len() - 2;
                        stack[parent_index].object_name = Some(text);
                    }
                    continue;
                }

                if is_full_text_search_element(&current.local_name)
                    && text.eq_ignore_ascii_case(USE_VALUE)
                {
                    let owner_names = owner_names(&stack);
                    if exclusions.preserves(&owner_names) {
                        continue;
                    }
                    let span_end = reader.buffer_position() as usize;
                    let span_start = span_end.saturating_sub(event.as_ref().len());
                    replacements.push(TextSpan {
                        start: span_start,
                        end: span_end,
                    });
                }
            }
            Ok(Event::CData(event)) => {
                if stack
                    .last()
                    .is_some_and(|context| is_full_text_search_element(&context.local_name))
                {
                    return Err(format!(
                        "{DISABLE_FULL_TEXT_SEARCH_RULE} supports only plain XML text values, not CDATA"
                    ));
                }
                let _ = event;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("failed to parse XML metadata: {error}")),
        }
    }

    Ok(replacements)
}

fn element_context(event: &BytesStart<'_>) -> ElementContext {
    ElementContext {
        local_name: local_name(event.name().as_ref()).to_owned(),
        object_name: attribute_value(event, "name"),
    }
}

fn attribute_value(event: &BytesStart<'_>, expected_name: &str) -> Option<String> {
    event
        .attributes()
        .with_checks(false)
        .filter_map(Result::ok)
        .find(|attribute| local_name(attribute.key.as_ref()).eq_ignore_ascii_case(expected_name))
        .map(|attribute| String::from_utf8_lossy(attribute.value.as_ref()).into_owned())
}

fn owner_names(stack: &[ElementContext]) -> Vec<OwnerName> {
    stack
        .iter()
        .filter_map(|context| {
            context.object_name.clone().map(|value| OwnerName {
                element_name: context.local_name.clone(),
                value,
            })
        })
        .collect()
}

fn replace_spans(input: &str, spans: &[TextSpan]) -> String {
    let mut output = String::with_capacity(input.len() + spans.len() * 4);
    let mut cursor = 0;
    for span in spans {
        output.push_str(&input[cursor..span.start]);
        output.push_str(DONT_USE_VALUE);
        cursor = span.end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn is_metadata_file(kind: SourceFileKind) -> bool {
    matches!(
        kind,
        SourceFileKind::ConfigurationMetadata
            | SourceFileKind::EdtMetadata
            | SourceFileKind::XmlMetadata
    )
}

fn is_name_element(name: &str) -> bool {
    name.eq_ignore_ascii_case("name")
}

fn is_full_text_search_element(name: &str) -> bool {
    name.eq_ignore_ascii_case("fullTextSearch")
}

fn is_tabular_section_context(name: &str) -> bool {
    name.to_ascii_lowercase().contains("tabularsection")
}

fn is_metadata_attribute_context(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "attribute"
            | "standardattribute"
            | "attributes"
            | "dimension"
            | "dimensions"
            | "property"
            | "properties"
            | "resource"
            | "resources"
    )
}

fn local_name(name: &[u8]) -> &str {
    let name = std::str::from_utf8(name).unwrap_or("");
    name.rsplit_once(':')
        .map(|(_prefix, local)| local)
        .unwrap_or(name)
}

fn normalize_metadata_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_owned()
}
