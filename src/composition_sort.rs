use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::Value;

use crate::scenario_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use crate::source_files::SourceFileKind;
use crate::xml_edt::parse_document;

pub const COMPOSITION_SORT_RULE: &str = "СортировкаСостава";
pub const METADATA_TREE_SORT_RULE: &str = "СортировкаДереваМетаданных";
pub const SUBSYSTEM_COMPOSITION_SORT_RULE: &str = "СортировкаСоставаПодсистем";

const DISABLED_OBJECTS_SETTING: &str = "ОтключенныеОбъекты";
const PREFIXES_SETTING: &str = "УчитываяПрефикс";

pub fn composition_sort(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    let settings = match CompositionSortSettings::from_settings(context.settings) {
        Ok(settings) => settings,
        Err(message) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                message,
            ));
        }
    };

    let scope = CompositionSortScope::for_rule_id(context.rule_id);
    if composition_target_for_path(&context.file.source_relative_path, context.file.kind, scope)
        .is_none()
    {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            scope.unsupported_file_message(),
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

    match sort_composition_text(
        &context.file.source_relative_path,
        context.file.kind,
        &input,
        &settings,
        scope,
    ) {
        CompositionSorting::Clean => ScenarioRun::clean(),
        CompositionSorting::Skipped(message) => ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            message,
        )),
        CompositionSorting::Modified(output) => match fs::write(&path, output) {
            Ok(()) => ScenarioRun::single(ScenarioResult::modified(
                context.rule_id,
                context.file.repo_path.clone(),
                "sorted metadata composition",
            )),
            Err(error) => ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                context.file.repo_path.clone(),
                format!("failed to write metadata composition file: {error}"),
            )),
        },
        CompositionSorting::Failed(message) => ScenarioRun::single(ScenarioResult::hard_failure(
            context.rule_id,
            context.file.repo_path.clone(),
            message,
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionSorting {
    Clean,
    Modified(String),
    Skipped(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionSortSettings {
    disabled_objects: Vec<String>,
    prefixes: Vec<String>,
}

impl CompositionSortSettings {
    pub fn from_settings(settings: Option<&Value>) -> Result<Self, String> {
        let disabled_objects = string_list_setting(settings, DISABLED_OBJECTS_SETTING)?;
        let prefixes = string_list_setting(settings, PREFIXES_SETTING)?;
        Ok(Self {
            disabled_objects,
            prefixes,
        })
    }

    fn is_disabled_for_target(&self, target: CompositionTarget) -> bool {
        let expected = match target {
            CompositionTarget::EdtConfiguration | CompositionTarget::DesignerConfiguration => {
                "конфигурация"
            }
            CompositionTarget::EdtSubsystem | CompositionTarget::DesignerSubsystem => "подсистема",
        };
        self.disabled_objects
            .iter()
            .any(|object| normalize_name(object) == expected)
    }
}

pub fn sort_composition_text(
    path: &Path,
    kind: SourceFileKind,
    input: &str,
    settings: &CompositionSortSettings,
    scope: CompositionSortScope,
) -> CompositionSorting {
    let Some(target) = composition_target_for_path(path, kind, scope) else {
        return CompositionSorting::Skipped(scope.unsupported_file_message().to_owned());
    };

    if settings.is_disabled_for_target(target) {
        return CompositionSorting::Skipped(
            match target {
                CompositionTarget::EdtConfiguration | CompositionTarget::DesignerConfiguration => {
                    "configuration composition sorting is disabled by scenario settings"
                }
                CompositionTarget::EdtSubsystem | CompositionTarget::DesignerSubsystem => {
                    "subsystem composition sorting is disabled by scenario settings"
                }
            }
            .to_owned(),
        );
    }

    if let Err(error) = parse_document(path, kind, input) {
        return CompositionSorting::Failed(error.to_string());
    }

    let blocks = collect_composition_blocks(input, target);
    let replacements = sorted_replacements(&blocks, settings);
    if replacements.is_empty() {
        return CompositionSorting::Clean;
    }

    let mut output = input.to_owned();
    for replacement in replacements.iter().rev() {
        output.replace_range(replacement.start..replacement.end, &replacement.source);
    }

    if output == input {
        CompositionSorting::Clean
    } else {
        CompositionSorting::Modified(output)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositionSortScope {
    All,
    MetadataTree,
    SubsystemComposition,
}

impl CompositionSortScope {
    fn for_rule_id(rule_id: &str) -> Self {
        match rule_id {
            METADATA_TREE_SORT_RULE => Self::MetadataTree,
            SUBSYSTEM_COMPOSITION_SORT_RULE => Self::SubsystemComposition,
            COMPOSITION_SORT_RULE => Self::All,
            _ => Self::All,
        }
    }

    fn unsupported_file_message(self) -> &'static str {
        match self {
            Self::All => {
                "scenario handles only configuration and subsystem metadata description files"
            }
            Self::MetadataTree => "scenario handles only Configuration.mdo and Configuration.xml",
            Self::SubsystemComposition => {
                "scenario handles only subsystem metadata description files"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompositionTarget {
    EdtConfiguration,
    DesignerConfiguration,
    EdtSubsystem,
    DesignerSubsystem,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompositionBlock {
    tag: String,
    source: String,
    value: String,
    start: usize,
    end: usize,
    sortable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Replacement {
    start: usize,
    end: usize,
    source: String,
}

#[derive(Debug)]
struct ActiveBlock {
    tag: String,
    start: usize,
    parent_depth: usize,
    text: String,
    has_nested_elements: bool,
}

fn collect_composition_blocks(input: &str, target: CompositionTarget) -> Vec<CompositionBlock> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut active = Option::<ActiveBlock>::None;
    let mut blocks = Vec::new();

    loop {
        let start = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                let tag = local_name(event.name().as_ref());
                if target_parent(&stack, target) && active.is_none() {
                    active = Some(ActiveBlock {
                        tag: tag.clone(),
                        start,
                        parent_depth: stack.len(),
                        text: String::new(),
                        has_nested_elements: false,
                    });
                } else if let Some(block) = active.as_mut()
                    && stack.len() > block.parent_depth
                {
                    block.has_nested_elements = true;
                }
                stack.push(tag);
            }
            Ok(Event::Empty(_event)) => {
                if let Some(block) = active.as_mut()
                    && stack.len() > block.parent_depth
                {
                    block.has_nested_elements = true;
                }
            }
            Ok(Event::Text(event)) => {
                if let Some(block) = active.as_mut()
                    && stack.len() == block.parent_depth + 1
                {
                    if let Ok(text) = event.decode() {
                        block.text.push_str(&text);
                    }
                }
            }
            Ok(Event::CData(_event)) => {
                if let Some(block) = active.as_mut()
                    && stack.len() == block.parent_depth + 1
                {
                    block.has_nested_elements = true;
                }
            }
            Ok(Event::End(event)) => {
                let end = reader.buffer_position() as usize;
                let tag = local_name(event.name().as_ref());
                let completes_active = active
                    .as_ref()
                    .is_some_and(|block| stack.len() == block.parent_depth + 1 && block.tag == tag);
                if completes_active {
                    let block = active.take().expect("active block was checked");
                    let value = block.text.trim().to_owned();
                    let sortable =
                        is_sortable_block(target, &block.tag, &value) && !block.has_nested_elements;
                    blocks.push(CompositionBlock {
                        tag: block.tag,
                        source: input[block.start..end].to_owned(),
                        value,
                        start: block.start,
                        end,
                        sortable,
                    });
                }
                stack.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => break,
        }
    }

    blocks
}

fn target_parent(stack: &[String], target: CompositionTarget) -> bool {
    match target {
        CompositionTarget::EdtConfiguration => stack.len() == 1 && stack[0] == "Configuration",
        CompositionTarget::DesignerConfiguration => matches!(
            stack,
            [root, configuration, child_objects]
                if root == "MetaDataObject"
                    && configuration == "Configuration"
                    && child_objects == "ChildObjects"
        ),
        CompositionTarget::EdtSubsystem => stack.len() == 1 && stack[0] == "Subsystem",
        CompositionTarget::DesignerSubsystem => matches!(
            stack,
            [root, subsystem, properties, content]
                if root == "MetaDataObject"
                    && subsystem == "Subsystem"
                    && properties == "Properties"
                    && content == "Content"
        ),
    }
}

fn is_sortable_block(target: CompositionTarget, tag: &str, value: &str) -> bool {
    if value.is_empty() || value.contains('-') {
        return false;
    }

    match target {
        CompositionTarget::EdtConfiguration => {
            if tag.eq_ignore_ascii_case("languages") || tag.eq_ignore_ascii_case("subsystems") {
                return false;
            }
            let Some((type_name, object_name)) = value.split_once('.') else {
                return false;
            };
            !object_name.is_empty() && type_name.chars().all(|ch| ch.is_ascii_alphabetic())
        }
        CompositionTarget::DesignerConfiguration => {
            !tag.eq_ignore_ascii_case("Language")
                && !tag.eq_ignore_ascii_case("Subsystem")
                && !value.is_empty()
        }
        CompositionTarget::EdtSubsystem => tag.eq_ignore_ascii_case("content"),
        CompositionTarget::DesignerSubsystem => tag.eq_ignore_ascii_case("Item"),
    }
}

fn sorted_replacements(
    blocks: &[CompositionBlock],
    settings: &CompositionSortSettings,
) -> Vec<Replacement> {
    let mut by_tag = BTreeMap::<&str, Vec<&CompositionBlock>>::new();
    for block in blocks.iter().filter(|block| block.sortable) {
        by_tag.entry(&block.tag).or_default().push(block);
    }

    let mut replacements = Vec::new();
    for group in by_tag.into_values() {
        let mut sorted = group.clone();
        sorted.sort_by(|left, right| {
            sort_key(&left.value, &settings.prefixes)
                .cmp(&sort_key(&right.value, &settings.prefixes))
        });

        for (slot, replacement) in group.iter().zip(sorted.iter()) {
            if slot.source != replacement.source {
                replacements.push(Replacement {
                    start: slot.start,
                    end: slot.end,
                    source: replacement.source.clone(),
                });
            }
        }
    }

    replacements.sort_by_key(|replacement| replacement.start);
    replacements
}

fn sort_key(value: &str, prefixes: &[String]) -> (usize, String) {
    let object_name = value
        .split_once('.')
        .map(|(_type_name, object_name)| object_name)
        .unwrap_or(value);
    let bucket = prefixes
        .iter()
        .position(|prefix| object_name.starts_with(prefix))
        .map(|index| index + 1)
        .unwrap_or(0);
    (bucket, value.to_owned())
}

fn string_list_setting(settings: Option<&Value>, name: &str) -> Result<Vec<String>, String> {
    let Some(settings) = settings else {
        return Ok(Vec::new());
    };
    let Some(value) = settings.get(name) else {
        return Ok(Vec::new());
    };

    match value {
        Value::String(value) => Ok(split_string_list(value)),
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(value) => Ok(value.trim().to_owned()),
                _ => Err(format!(
                    "{COMPOSITION_SORT_RULE}.{name} must contain only strings"
                )),
            })
            .filter_map(|result| match result {
                Ok(value) if value.is_empty() => None,
                other => Some(other),
            })
            .collect(),
        _ => Err(format!(
            "{COMPOSITION_SORT_RULE}.{name} must be a string or an array of strings"
        )),
    }
}

fn split_string_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn normalize_name(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
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

fn composition_target_for_path(
    path: &Path,
    kind: SourceFileKind,
    scope: CompositionSortScope,
) -> Option<CompositionTarget> {
    let target = if is_edt_configuration_path(path) {
        Some(CompositionTarget::EdtConfiguration)
    } else if is_designer_configuration_path(path) {
        Some(CompositionTarget::DesignerConfiguration)
    } else if is_edt_subsystem_path(path, kind) {
        Some(CompositionTarget::EdtSubsystem)
    } else if is_designer_subsystem_path(path, kind) {
        Some(CompositionTarget::DesignerSubsystem)
    } else {
        None
    }?;

    match (scope, target) {
        (
            CompositionSortScope::All,
            CompositionTarget::EdtConfiguration
            | CompositionTarget::DesignerConfiguration
            | CompositionTarget::EdtSubsystem
            | CompositionTarget::DesignerSubsystem,
        ) => Some(target),
        (
            CompositionSortScope::MetadataTree,
            CompositionTarget::EdtConfiguration | CompositionTarget::DesignerConfiguration,
        ) => Some(target),
        (
            CompositionSortScope::SubsystemComposition,
            CompositionTarget::EdtSubsystem | CompositionTarget::DesignerSubsystem,
        ) => Some(target),
        _ => None,
    }
}

fn is_edt_subsystem_path(path: &Path, kind: SourceFileKind) -> bool {
    if kind != SourceFileKind::EdtMetadata {
        return false;
    }

    let components = path_components(path);
    let Some(file_name) = components.last() else {
        return false;
    };
    let Some(parent_name) = components.get(components.len().saturating_sub(2)) else {
        return false;
    };
    if !file_name.eq_ignore_ascii_case(&format!("{parent_name}.mdo")) {
        return false;
    }

    components
        .iter()
        .any(|component| component.eq_ignore_ascii_case("Subsystems"))
}

fn is_designer_subsystem_path(path: &Path, kind: SourceFileKind) -> bool {
    if kind != SourceFileKind::XmlMetadata {
        return false;
    }

    let components = path_components(path);
    let Some(file_name) = components.last() else {
        return false;
    };
    if !file_name.to_ascii_lowercase().ends_with(".xml") {
        return false;
    }

    components
        .get(components.len().saturating_sub(2))
        .is_some_and(|parent| parent.eq_ignore_ascii_case("Subsystems"))
}

fn path_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::to_owned)
        .collect()
}

fn local_name(name: &[u8]) -> String {
    let name = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map(|index| &name[index + 1..])
        .unwrap_or(name);
    String::from_utf8_lossy(name).into_owned()
}
