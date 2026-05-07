use std::collections::BTreeMap;
use std::fs;
use std::ops::Range;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::xml_edt::parse_document;
use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::SourceFileKind;

pub const DUPLICATE_METADATA_RULE: &str = "УдалениеДублейМетаданных";
pub const DUPLICATE_METADATA_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    DUPLICATE_METADATA_RULE,
    "УдалениеДублейМетаданных.os",
    duplicate_metadata,
);

pub fn duplicate_metadata(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if duplicate_metadata_format_for_path(&context.file.source_relative_path).is_none() {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only Configuration.mdo and Configuration.xml",
        ));
    }

    let path = context.repo_root.join(&context.file.repo_path);
    let input = match fs::read_to_string(&path) {
        Ok(input) => input,
        Err(error) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to read metadata composition file: {error}"),
            ));
        }
    };

    match remove_duplicate_metadata_text(
        &context.file.source_relative_path,
        context.file.kind,
        &input,
    ) {
        DuplicateMetadataRemoval::Clean => ScenarioRun::clean(),
        DuplicateMetadataRemoval::Skipped(message) => ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            message,
        )),
        DuplicateMetadataRemoval::Modified(output) => match fs::write(&path, output) {
            Ok(()) => ScenarioRun::single(ScenarioResult::modified(
                context.rule_id,
                context.file.repo_path.clone(),
                "removed duplicate metadata entries",
            )),
            Err(error) => ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to write metadata composition file: {error}"),
            )),
        },
        DuplicateMetadataRemoval::Failed(message) => ScenarioRun::single(
            ScenarioResult::hard_failure(context.rule_id, context.file.repo_path.clone(), message),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateMetadataRemoval {
    Clean,
    Modified(String),
    Skipped(String),
    Failed(String),
}

pub fn remove_duplicate_metadata_text(
    path: &Path,
    kind: SourceFileKind,
    input: &str,
) -> DuplicateMetadataRemoval {
    let Some(format) = duplicate_metadata_format_for_path(path) else {
        return DuplicateMetadataRemoval::Skipped(
            "scenario handles only Configuration.mdo and Configuration.xml".to_owned(),
        );
    };

    if let Err(error) = parse_document(path, kind, input) {
        return DuplicateMetadataRemoval::Failed(error.to_string());
    }

    let duplicate_ranges = duplicate_metadata_ranges(input, format);
    if duplicate_ranges.is_empty() {
        return DuplicateMetadataRemoval::Clean;
    }

    let mut output = input.to_owned();
    for range in duplicate_ranges.iter().rev() {
        output.replace_range(range.clone(), "");
    }

    if output == input {
        DuplicateMetadataRemoval::Clean
    } else {
        DuplicateMetadataRemoval::Modified(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DuplicateMetadataFormat {
    Edt,
    Designer,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DuplicateMetadataKey {
    tag: String,
    value: String,
    source: String,
}

#[derive(Debug)]
struct ActiveEntry {
    tag: String,
    start: usize,
    parent_depth: usize,
    text: String,
    has_nested_elements: bool,
}

#[derive(Debug)]
struct MetadataEntry {
    key: DuplicateMetadataKey,
    start: usize,
    end: usize,
}

fn duplicate_metadata_ranges(input: &str, format: DuplicateMetadataFormat) -> Vec<Range<usize>> {
    let entries = collect_metadata_entries(input, format);
    let mut totals = BTreeMap::<DuplicateMetadataKey, usize>::new();
    for entry in &entries {
        *totals.entry(entry.key.clone()).or_default() += 1;
    }

    let mut seen = BTreeMap::<DuplicateMetadataKey, usize>::new();
    let mut ranges = Vec::new();

    for entry in entries {
        let total = totals
            .get(&entry.key)
            .copied()
            .expect("entry total was counted");
        let seen_count = seen.entry(entry.key).or_default();
        *seen_count += 1;
        if *seen_count == total {
            continue;
        }
        ranges.push(expanded_removal_range(input, entry.start, entry.end));
    }

    ranges.sort_by_key(|range| range.start);
    ranges
}

fn collect_metadata_entries(input: &str, format: DuplicateMetadataFormat) -> Vec<MetadataEntry> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut active = Option::<ActiveEntry>::None;
    let mut entries = Vec::new();
    let mut edt_after_languages = false;

    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if target_parent(&stack, format, edt_after_languages) && active.is_none() {
                    active = Some(ActiveEntry {
                        tag: tag.clone(),
                        start,
                        parent_depth: stack.len(),
                        text: String::new(),
                        has_nested_elements: false,
                    });
                } else if let Some(entry) = active.as_mut()
                    && stack.len() > entry.parent_depth
                {
                    entry.has_nested_elements = true;
                }
                stack.push(tag);
            }
            Ok(Event::Empty(_event)) => {
                if let Some(entry) = active.as_mut()
                    && stack.len() > entry.parent_depth
                {
                    entry.has_nested_elements = true;
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(entry) = active.as_mut()
                    && stack.len() == entry.parent_depth + 1
                {
                    if let Ok(text) = event.decode() {
                        entry.text.push_str(&text);
                    }
                }
            }
            Ok(Event::CData(_event)) => {
                if let Some(entry) = active.as_mut()
                    && stack.len() == entry.parent_depth + 1
                {
                    entry.has_nested_elements = true;
                }
            }
            Ok(Event::End(event)) => {
                let end = reader.buffer_position() as usize;
                let tag = local_name(event.name().as_ref());
                if format == DuplicateMetadataFormat::Edt
                    && stack.len() == 2
                    && stack[0] == "Configuration"
                    && tag == "languages"
                {
                    edt_after_languages = true;
                }
                let completes_active = active
                    .as_ref()
                    .is_some_and(|entry| stack.len() == entry.parent_depth + 1 && entry.tag == tag);
                if completes_active {
                    let entry = active.take().expect("active entry was checked");
                    let value = entry.text.trim().to_owned();
                    if !entry.has_nested_elements && is_duplicate_candidate(format, &value) {
                        let key_source_start = element_line_start(input, entry.start);
                        entries.push(MetadataEntry {
                            key: DuplicateMetadataKey {
                                tag: entry.tag,
                                value,
                                source: input[key_source_start..end].to_owned(),
                            },
                            start: entry.start,
                            end,
                        });
                    }
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    entries
}

fn expanded_removal_range(input: &str, start: usize, end: usize) -> Range<usize> {
    let line_start = element_line_start(input, start);
    let prefix = &input[line_start..start];
    if prefix
        .chars()
        .all(|character| matches!(character, ' ' | '\t'))
    {
        let mut line_end = end;
        if input[end..].starts_with("\r\n") {
            line_end += 2;
        } else if input[end..].starts_with('\n') {
            line_end += 1;
        }
        line_start..line_end
    } else {
        start..end
    }
}

fn element_line_start(input: &str, start: usize) -> usize {
    let line_start = input[..start]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = &input[line_start..start];
    if prefix
        .chars()
        .all(|character| matches!(character, ' ' | '\t'))
    {
        line_start
    } else {
        start
    }
}

fn target_parent(
    stack: &[String],
    format: DuplicateMetadataFormat,
    edt_after_languages: bool,
) -> bool {
    match format {
        DuplicateMetadataFormat::Edt => {
            edt_after_languages && stack.len() == 1 && stack[0] == "Configuration"
        }
        DuplicateMetadataFormat::Designer => matches!(
            stack,
            [root, configuration, child_objects]
                if root == "MetaDataObject"
                    && configuration == "Configuration"
                    && child_objects == "ChildObjects"
        ),
    }
}

fn is_duplicate_candidate(format: DuplicateMetadataFormat, value: &str) -> bool {
    if value.is_empty() || value.contains('-') {
        return false;
    }

    match format {
        DuplicateMetadataFormat::Edt => {
            let Some((type_name, object_name)) = value.split_once('.') else {
                return false;
            };
            !object_name.is_empty()
                && type_name
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())
        }
        DuplicateMetadataFormat::Designer => true,
    }
}

fn is_edt_configuration_path(path: &Path) -> bool {
    let components = path.components().collect::<Vec<_>>();
    components.len() == 2
        && components[0].as_os_str() == "Configuration"
        && components[1].as_os_str() == "Configuration.mdo"
}

fn is_designer_configuration_path(path: &Path) -> bool {
    let components = path.components().collect::<Vec<_>>();
    components.len() == 1 && components[0].as_os_str() == "Configuration.xml"
}

fn duplicate_metadata_format_for_path(path: &Path) -> Option<DuplicateMetadataFormat> {
    if is_edt_configuration_path(path) {
        Some(DuplicateMetadataFormat::Edt)
    } else if is_designer_configuration_path(path) {
        Some(DuplicateMetadataFormat::Designer)
    } else {
        None
    }
}

fn local_name(name: &[u8]) -> String {
    let name = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map(|index| &name[index + 1..])
        .unwrap_or(name);
    String::from_utf8_lossy(name).into_owned()
}
