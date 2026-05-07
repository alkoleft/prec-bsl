use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::xml_edt::{parse_document, write_validated_xml};
use prec_bsl_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use prec_bsl_source::SourceFileKind;

pub const XML_FORM_CORRECTION_RULE: &str = "КорректировкаXMLФорм";

const FORM_FILE_NAME: &str = "Form.form";
const BASE_FORM_DIR: &str = "BaseForm";

pub fn xml_form_correction(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if context.file.kind != SourceFileKind::EdtForm || !is_edt_form_file(&context.file.repo_path) {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only EDT Form.form files",
        ));
    }

    if is_base_form_file(&context.file.repo_path) {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "base form files are processed through the owning Form.form",
        ));
    }

    let mut results = Vec::new();
    let base_repo_path = base_form_repo_path(&context.file.repo_path);
    let base_absolute_path = context.repo_root.join(&base_repo_path);

    let base_elements = if base_absolute_path.is_file() {
        match correct_form_file(context.repo_root, &base_repo_path, None) {
            FormCorrection::Clean(elements) => Some(elements),
            FormCorrection::Modified { elements } => {
                results.push(ScenarioResult::modified(
                    context.rule_id,
                    base_repo_path.clone(),
                    "corrected duplicate XML form element ids",
                ));
                Some(elements)
            }
            FormCorrection::Failed(message) => {
                return ScenarioRun::single(ScenarioResult::hard_failure(
                    context.rule_id,
                    base_repo_path,
                    message,
                ));
            }
        }
    } else {
        None
    };

    match correct_form_file(
        context.repo_root,
        &context.file.repo_path,
        base_elements.as_deref(),
    ) {
        FormCorrection::Clean(_) => {}
        FormCorrection::Modified { .. } => results.push(ScenarioResult::modified(
            context.rule_id,
            context.file.repo_path.clone(),
            "corrected duplicate XML form element ids",
        )),
        FormCorrection::Failed(message) => results.push(ScenarioResult::hard_failure(
            context.rule_id,
            context.file.repo_path.clone(),
            message,
        )),
    }

    ScenarioRun {
        results,
        post_processing_paths: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlFormCorrection {
    Clean,
    Modified(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormElement {
    pub path: String,
    pub name: String,
    pub id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditableFormElement {
    path: String,
    name: String,
    id: u64,
    new_id: u64,
    id_span: TextSpan,
    borrowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FormCorrection {
    Clean(Vec<FormElement>),
    Modified { elements: Vec<FormElement> },
    Failed(String),
}

fn correct_form_file(
    repo_root: &Path,
    repo_path: &Path,
    base_elements: Option<&[FormElement]>,
) -> FormCorrection {
    let absolute_path = repo_root.join(repo_path);
    let input = match fs::read_to_string(&absolute_path) {
        Ok(input) => input,
        Err(error) => {
            return FormCorrection::Failed(format!("failed to read file: {error}"));
        }
    };

    match correct_edt_form_text(repo_path, &input, base_elements) {
        XmlFormCorrection::Clean => FormCorrection::Clean(match collect_form_elements(&input) {
            Ok(elements) => elements.into_iter().map(FormElement::from).collect(),
            Err(message) => return FormCorrection::Failed(message),
        }),
        XmlFormCorrection::Modified(output) => {
            if let Err(error) = fs::write(&absolute_path, &output) {
                return FormCorrection::Failed(format!("failed to write file: {error}"));
            }
            match collect_form_elements(&output) {
                Ok(elements) => FormCorrection::Modified {
                    elements: elements.into_iter().map(FormElement::from).collect(),
                },
                Err(message) => FormCorrection::Failed(message),
            }
        }
        XmlFormCorrection::Failed(message) => FormCorrection::Failed(message),
    }
}

pub fn correct_edt_form_text(
    repo_path: &Path,
    input: &str,
    base_elements: Option<&[FormElement]>,
) -> XmlFormCorrection {
    if let Err(error) = parse_document(repo_path, SourceFileKind::EdtForm, input) {
        return XmlFormCorrection::Failed(error.to_string());
    }

    let mut elements = match collect_form_elements(input) {
        Ok(elements) => elements,
        Err(message) => return XmlFormCorrection::Failed(message),
    };

    if let Some(base_elements) = base_elements {
        if let Err(message) = restore_base_form_links(&mut elements, base_elements) {
            return XmlFormCorrection::Failed(message);
        }
    }

    replace_duplicate_ids(&mut elements);

    if elements.iter().all(|element| element.id == element.new_id) {
        return XmlFormCorrection::Clean;
    }

    let output = replace_id_lines(input, &elements);
    if let Err(error) = write_validated_xml(repo_path, &output) {
        return XmlFormCorrection::Failed(error.to_string());
    }

    XmlFormCorrection::Modified(output)
}

fn collect_form_elements(input: &str) -> Result<Vec<EditableFormElement>, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut elements = Vec::new();
    let mut stack = Vec::new();
    let mut current_name = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(tag_name(event.name().as_ref())),
            Ok(Event::Empty(_)) => {}
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Text(event)) => {
                let Some(tag) = stack.last().cloned() else {
                    continue;
                };
                let value = event
                    .decode()
                    .map_err(|error| format!("failed to decode XML form text: {error}"))?
                    .into_owned();

                if tag == "name" {
                    current_name = value;
                    if stack.len() >= 2 {
                        let parent_index = stack.len() - 2;
                        stack[parent_index] = format!("{}.{}", stack[parent_index], current_name);
                    }
                } else if tag == "id" && !current_name.is_empty() {
                    let id_text = value.trim();
                    let id = id_text.parse::<u64>().map_err(|_| {
                        format!("form element id must be a positive integer: {id_text}")
                    })?;
                    if id == 0 {
                        return Err("form element id must be a positive integer: 0".to_owned());
                    }
                    let span_end = reader.buffer_position() as usize;
                    let span_start = span_end.saturating_sub(event.as_ref().len());
                    elements.push(EditableFormElement {
                        path: stack[..stack.len().saturating_sub(1)].join("."),
                        name: current_name.clone(),
                        id,
                        new_id: id,
                        id_span: TextSpan {
                            start: span_start,
                            end: span_end,
                        },
                        borrowed: false,
                    });
                }
            }
            Ok(Event::CData(event)) => {
                let Some(tag) = stack.last() else {
                    continue;
                };
                if tag == "id" || tag == "name" {
                    return Err("form element name and id must be plain XML text".to_owned());
                }
                let _ = event;
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("failed to parse XML form: {error}")),
        }
    }

    Ok(elements)
}

fn restore_base_form_links(
    elements: &mut [EditableFormElement],
    base_elements: &[FormElement],
) -> Result<(), String> {
    for base_element in base_elements {
        let matching_indexes = elements
            .iter()
            .enumerate()
            .filter_map(|(index, element)| {
                (element.path == base_element.path && element.name == base_element.name)
                    .then_some(index)
            })
            .collect::<Vec<_>>();

        match matching_indexes.as_slice() {
            [] => {}
            [index] => {
                let element = &mut elements[*index];
                element.new_id = base_element.id;
                element.borrowed = true;
            }
            _ => {
                return Err(format!(
                    "base form element match is ambiguous: {}",
                    base_element.path
                ));
            }
        }
    }

    Ok(())
}

fn replace_duplicate_ids(elements: &mut [EditableFormElement]) {
    let mut groups = duplicate_groups(elements);
    groups.sort_by(|left, right| {
        right
            .len()
            .cmp(&left.len())
            .then_with(|| elements[left[0]].new_id.cmp(&elements[right[0]].new_id))
    });

    let mut free_ids = FreeIdAllocator::new(elements);

    for group in groups {
        let has_borrowed_element = group.iter().any(|index| elements[*index].borrowed);
        let indexes_to_replace = if has_borrowed_element {
            group
                .into_iter()
                .filter(|index| !elements[*index].borrowed)
                .collect::<Vec<_>>()
        } else {
            group[..group.len() - 1].to_vec()
        };

        for index in indexes_to_replace {
            elements[index].new_id = free_ids.next_id();
        }
    }
}

fn duplicate_groups(elements: &[EditableFormElement]) -> Vec<Vec<usize>> {
    let mut by_id: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (index, element) in elements.iter().enumerate() {
        by_id.entry(element.new_id).or_default().push(index);
    }
    by_id
        .into_values()
        .filter(|indexes| indexes.len() > 1)
        .collect()
}

#[derive(Debug, Clone)]
struct FreeIdAllocator {
    used_ids: BTreeSet<u64>,
    candidate: u64,
    next_after_max: Option<u64>,
}

impl FreeIdAllocator {
    fn new(elements: &[EditableFormElement]) -> Self {
        let used_ids = elements
            .iter()
            .map(|element| element.new_id)
            .collect::<BTreeSet<_>>();
        let next_after_max = used_ids
            .iter()
            .next_back()
            .map(|id| id.checked_add(1))
            .unwrap_or(Some(1));

        Self {
            used_ids,
            candidate: 1,
            next_after_max,
        }
    }

    fn next_id(&mut self) -> u64 {
        if let Some(next_after_max) = self.next_after_max {
            while self.used_ids.contains(&self.candidate) && self.candidate < next_after_max {
                self.candidate += 1;
            }

            let id = if self.candidate < next_after_max {
                let id = self.candidate;
                self.candidate += 1;
                id
            } else {
                let id = next_after_max;
                self.next_after_max = id.checked_add(1);
                id
            };
            self.used_ids.insert(id);
            return id;
        }

        while self.used_ids.contains(&self.candidate) {
            self.candidate = self
                .candidate
                .checked_add(1)
                .expect("finite form element set must leave at least one free u64 id");
        }
        let id = self.candidate;
        self.candidate = self.candidate.checked_add(1).unwrap_or(self.candidate);
        self.used_ids.insert(id);
        id
    }
}

fn replace_id_lines(input: &str, elements: &[EditableFormElement]) -> String {
    let mut replacements = elements
        .iter()
        .filter(|element| element.id != element.new_id)
        .map(|element| (element.id_span, element.new_id.to_string()))
        .collect::<Vec<_>>();
    replacements.sort_by_key(|(span, _)| span.start);

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    for (span, replacement) in replacements {
        output.push_str(&input[cursor..span.start]);
        output.push_str(&replacement);
        cursor = span.end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn tag_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn is_edt_form_file(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == FORM_FILE_NAME)
}

fn is_base_form_file(path: &Path) -> bool {
    path.parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == BASE_FORM_DIR)
}

fn base_form_repo_path(path: &Path) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new(""))
        .join(BASE_FORM_DIR)
        .join(FORM_FILE_NAME)
}

impl From<EditableFormElement> for FormElement {
    fn from(element: EditableFormElement) -> Self {
        Self {
            path: element.path,
            name: element.name,
            id: element.new_id,
        }
    }
}
