use std::fs;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::xml_edt::{parse_document, write_validated_xml};
use prec_bsl_pipeline::{
    ScenarioDefinition, ScenarioExecutionContext, ScenarioResult, ScenarioRun,
};
use prec_bsl_source::SourceFileKind;

pub const DISABLE_FORM_CHANGE_RULE: &str = "ОтключениеРазрешенияИзменятьФорму";
pub const DISABLE_FORM_CHANGE_SCENARIO: ScenarioDefinition = ScenarioDefinition::required_v1(
    DISABLE_FORM_CHANGE_RULE,
    "ОтключениеРазрешенияИзменятьФорму.os",
    disable_form_change_permission,
);

const TRUE_VALUE: &str = "true";
const FALSE_VALUE: &str = "false";

pub fn disable_form_change_permission(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    if !is_form_description_file(&context.file.repo_path) {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only XML form description files",
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

    match disable_form_change_permission_text(&context.file.repo_path, context.file.kind, &input) {
        FormChangePermissionDisabling::Clean => ScenarioRun::clean(),
        FormChangePermissionDisabling::Modified(output) => {
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
                "disabled form customization permission",
            ))
        }
        FormChangePermissionDisabling::Failed(message) => ScenarioRun::single(
            ScenarioResult::hard_failure(context.rule_id, context.file.repo_path.clone(), message),
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormChangePermissionDisabling {
    Clean,
    Modified(String),
    Failed(String),
}

pub fn disable_form_change_permission_text(
    repo_path: &Path,
    kind: SourceFileKind,
    input: &str,
) -> FormChangePermissionDisabling {
    if let Err(error) = parse_document(repo_path, kind, input) {
        return FormChangePermissionDisabling::Failed(error.to_string());
    }

    let change = match kind {
        SourceFileKind::EdtForm if is_edt_form_path(repo_path) => collect_edt_change(input),
        SourceFileKind::XmlMetadata if is_designer_form_path(repo_path) => {
            collect_designer_change(input)
        }
        _ => Ok(XmlChange::Clean),
    };

    let output = match change {
        Ok(XmlChange::Clean) => return FormChangePermissionDisabling::Clean,
        Ok(XmlChange::Modified(output)) => output,
        Err(message) => return FormChangePermissionDisabling::Failed(message),
    };

    if let Err(error) = write_validated_xml(repo_path, &output) {
        return FormChangePermissionDisabling::Failed(error.to_string());
    }

    FormChangePermissionDisabling::Modified(output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum XmlChange {
    Clean,
    Modified(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TextSpan {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ElementFrame {
    name: String,
    has_text: bool,
}

fn collect_edt_change(input: &str) -> Result<XmlChange, String> {
    let replacements = collect_boolean_property_replacements(input, "allowFormCustomize")?;
    if replacements.is_empty() {
        Ok(XmlChange::Clean)
    } else {
        Ok(XmlChange::Modified(replace_spans(input, &replacements)))
    }
}

fn collect_designer_change(input: &str) -> Result<XmlChange, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);

    let mut stack = Vec::<ElementFrame>::new();
    let mut replacements = Vec::new();
    let mut customizable_seen = false;
    let mut window_opening_mode_end = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case("Customizable"))
                {
                    return Err("Customizable must contain true or false plain text".to_owned());
                }
                let name = local_name(event.name().as_ref()).to_owned();
                if name.eq_ignore_ascii_case("Customizable") {
                    customizable_seen = true;
                }
                stack.push(ElementFrame {
                    name,
                    has_text: false,
                });
            }
            Ok(Event::Empty(event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case("Customizable"))
                {
                    return Err("Customizable must contain true or false plain text".to_owned());
                }
                let name = local_name(event.name().as_ref()).to_owned();
                if name.eq_ignore_ascii_case("Customizable") {
                    return Err("Customizable must contain true or false plain text".to_owned());
                }
                if name.eq_ignore_ascii_case("WindowOpeningMode") {
                    window_opening_mode_end = Some(reader.buffer_position() as usize);
                }
            }
            Ok(Event::End(event)) => {
                let name = local_name(event.name().as_ref()).to_owned();
                if name.eq_ignore_ascii_case("WindowOpeningMode") {
                    window_opening_mode_end = Some(reader.buffer_position() as usize);
                }
                let Some(frame) = stack.pop() else {
                    continue;
                };
                if frame.name.eq_ignore_ascii_case("Customizable") && !frame.has_text {
                    return Err("Customizable must contain true or false plain text".to_owned());
                }
            }
            Ok(Event::Text(event)) => {
                let Some(current) = stack.last() else {
                    continue;
                };
                if !current.name.eq_ignore_ascii_case("Customizable") {
                    continue;
                }

                let text = event
                    .decode()
                    .map_err(|error| format!("failed to decode Customizable text: {error}"))?
                    .into_owned();
                if let Some(current) = stack.last_mut() {
                    current.has_text = true;
                }
                collect_boolean_replacement(
                    &mut replacements,
                    &text,
                    event.as_ref().len(),
                    reader.buffer_position() as usize,
                    "Customizable",
                )?;
            }
            Ok(Event::CData(_event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case("Customizable"))
                {
                    return Err(
                        "Customizable supports only plain XML text values, not CDATA".to_owned(),
                    );
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("failed to parse XML form: {error}")),
        }
    }

    if !replacements.is_empty() {
        return Ok(XmlChange::Modified(replace_spans(input, &replacements)));
    }
    if customizable_seen {
        return Ok(XmlChange::Clean);
    }
    if let Some(anchor_end) = window_opening_mode_end {
        return Ok(XmlChange::Modified(insert_customizable_after_anchor(
            input, anchor_end,
        )));
    }

    Ok(XmlChange::Clean)
}

fn collect_boolean_property_replacements(
    input: &str,
    property_name: &str,
) -> Result<Vec<TextSpan>, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<ElementFrame>::new();
    let mut replacements = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case(property_name))
                {
                    return Err(format!(
                        "{property_name} must contain true or false plain text"
                    ));
                }
                stack.push(ElementFrame {
                    name: local_name(event.name().as_ref()).to_owned(),
                    has_text: false,
                });
            }
            Ok(Event::Empty(event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case(property_name))
                {
                    return Err(format!(
                        "{property_name} must contain true or false plain text"
                    ));
                }
                if local_name(event.name().as_ref()).eq_ignore_ascii_case(property_name) {
                    return Err(format!(
                        "{property_name} must contain true or false plain text"
                    ));
                }
            }
            Ok(Event::End(_event)) => {
                let Some(frame) = stack.pop() else {
                    continue;
                };
                if frame.name.eq_ignore_ascii_case(property_name) && !frame.has_text {
                    return Err(format!(
                        "{property_name} must contain true or false plain text"
                    ));
                }
            }
            Ok(Event::Text(event)) => {
                let Some(current) = stack.last() else {
                    continue;
                };
                if !current.name.eq_ignore_ascii_case(property_name) {
                    continue;
                }
                let text = event
                    .decode()
                    .map_err(|error| format!("failed to decode {property_name} text: {error}"))?
                    .into_owned();
                if let Some(current) = stack.last_mut() {
                    current.has_text = true;
                }
                collect_boolean_replacement(
                    &mut replacements,
                    &text,
                    event.as_ref().len(),
                    reader.buffer_position() as usize,
                    property_name,
                )?;
            }
            Ok(Event::CData(_event)) => {
                if stack
                    .last()
                    .is_some_and(|frame| frame.name.eq_ignore_ascii_case(property_name))
                {
                    return Err(format!(
                        "{property_name} supports only plain XML text values, not CDATA"
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(format!("failed to parse XML form: {error}")),
        }
    }

    Ok(replacements)
}

fn collect_boolean_replacement(
    replacements: &mut Vec<TextSpan>,
    text: &str,
    text_len: usize,
    span_end: usize,
    property_name: &str,
) -> Result<(), String> {
    if text.eq_ignore_ascii_case(TRUE_VALUE) {
        replacements.push(TextSpan {
            start: span_end.saturating_sub(text_len),
            end: span_end,
        });
        Ok(())
    } else if text.eq_ignore_ascii_case(FALSE_VALUE) {
        Ok(())
    } else {
        Err(format!(
            "{property_name} must contain true or false, got {text:?}"
        ))
    }
}

fn replace_spans(input: &str, spans: &[TextSpan]) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    for span in spans {
        output.push_str(&input[cursor..span.start]);
        output.push_str(FALSE_VALUE);
        cursor = span.end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn insert_customizable_after_anchor(input: &str, anchor_end: usize) -> String {
    let line_start = input[..anchor_end]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let indent = input[line_start..]
        .chars()
        .take_while(|character| *character == ' ' || *character == '\t')
        .collect::<String>();
    let line_ending = if input[anchor_end..].starts_with("\r\n") {
        "\r\n"
    } else if input[anchor_end..].starts_with('\n') {
        "\n"
    } else if input[..anchor_end].contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut output = String::with_capacity(input.len() + 40 + indent.len());
    output.push_str(&input[..anchor_end]);
    output.push_str(line_ending);
    output.push_str(&indent);
    output.push_str("<Customizable>false</Customizable>");
    output.push_str(&input[anchor_end..]);
    output
}

fn is_form_description_file(path: &Path) -> bool {
    is_edt_form_path(path) || is_designer_form_path(path)
}

fn is_edt_form_path(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "Form.form")
}

fn is_designer_form_path(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "Form.xml")
}

fn local_name(name: &[u8]) -> &str {
    let name = std::str::from_utf8(name).unwrap_or("");
    name.rsplit_once(':')
        .map(|(_prefix, local)| local)
        .unwrap_or(name)
}
