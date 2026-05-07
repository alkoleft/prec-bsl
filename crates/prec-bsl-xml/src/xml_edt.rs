use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

use prec_bsl_source::{SourceFile, SourceFileKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlEdtDocument {
    pub path: PathBuf,
    pub kind: SourceFileKind,
    pub root_element: String,
    source: String,
}

impl XmlEdtDocument {
    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Debug)]
pub enum XmlEdtError {
    UnsupportedKind {
        path: PathBuf,
        kind: SourceFileKind,
    },
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        position: u64,
        source: quick_xml::errors::Error,
    },
    Write {
        path: PathBuf,
        source: io::Error,
    },
    MissingRoot {
        path: PathBuf,
    },
    InvalidDocument {
        path: PathBuf,
        position: u64,
        message: String,
    },
}

impl fmt::Display for XmlEdtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedKind { path, kind } => {
                write!(
                    formatter,
                    "file is not supported by XML/EDT parser: {} ({kind:?})",
                    path.display()
                )
            }
            Self::Read { path, source } => {
                write!(
                    formatter,
                    "failed to read XML/EDT file {}: {source}",
                    path.display()
                )
            }
            Self::Parse {
                path,
                position,
                source,
            } => {
                write!(
                    formatter,
                    "failed to parse XML/EDT file {} at byte {position}: {source}",
                    path.display()
                )
            }
            Self::Write { path, source } => {
                write!(
                    formatter,
                    "failed to write XML/EDT document {}: {source}",
                    path.display()
                )
            }
            Self::MissingRoot { path } => {
                write!(
                    formatter,
                    "failed to parse XML/EDT file {}: document has no root element",
                    path.display()
                )
            }
            Self::InvalidDocument {
                path,
                position,
                message,
            } => {
                write!(
                    formatter,
                    "failed to parse XML/EDT file {} at byte {position}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for XmlEdtError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } | Self::Write { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::UnsupportedKind { .. }
            | Self::MissingRoot { .. }
            | Self::InvalidDocument { .. } => None,
        }
    }
}

pub fn read_document(repo_root: &Path, file: &SourceFile) -> Result<XmlEdtDocument, XmlEdtError> {
    ensure_xml_edt_kind(&file.repo_path, file.kind)?;
    let path = repo_root.join(&file.repo_path);
    let source = fs::read_to_string(&path).map_err(|source| XmlEdtError::Read {
        path: file.repo_path.clone(),
        source,
    })?;
    parse_document(file.repo_path.clone(), file.kind, source)
}

pub fn parse_document(
    path: impl Into<PathBuf>,
    kind: SourceFileKind,
    source: impl Into<String>,
) -> Result<XmlEdtDocument, XmlEdtError> {
    let path = path.into();
    ensure_xml_edt_kind(&path, kind)?;
    let source = source.into();
    let root_element = validate_xml(&path, &source)?;
    Ok(XmlEdtDocument {
        path,
        kind,
        root_element,
        source,
    })
}

pub fn write_document(document: &XmlEdtDocument) -> Result<String, XmlEdtError> {
    write_validated_xml(&document.path, document.source())
}

pub fn write_validated_xml(path: &Path, source: &str) -> Result<String, XmlEdtError> {
    validate_xml(path, source)?;

    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::with_capacity(source.len()));

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(event) => {
                writer
                    .write_event(event.borrow())
                    .map_err(|source| XmlEdtError::Write {
                        path: path.to_path_buf(),
                        source,
                    })?;
            }
            Err(source) => {
                return Err(XmlEdtError::Parse {
                    path: path.to_path_buf(),
                    position: reader.error_position(),
                    source,
                });
            }
        }
    }

    String::from_utf8(writer.into_inner()).map_err(|source| XmlEdtError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidData, source),
    })
}

fn ensure_xml_edt_kind(path: &Path, kind: SourceFileKind) -> Result<(), XmlEdtError> {
    if is_xml_edt_kind(kind) {
        Ok(())
    } else {
        Err(XmlEdtError::UnsupportedKind {
            path: path.to_path_buf(),
            kind,
        })
    }
}

fn is_xml_edt_kind(kind: SourceFileKind) -> bool {
    matches!(
        kind,
        SourceFileKind::ConfigurationMetadata
            | SourceFileKind::EdtMetadata
            | SourceFileKind::EdtForm
            | SourceFileKind::XmlMetadata
    )
}

fn validate_xml(path: &Path, source: &str) -> Result<String, XmlEdtError> {
    let mut reader = Reader::from_str(source);
    reader.config_mut().trim_text(false);
    let mut root_element = None;
    let mut depth = 0usize;
    let mut seen_before_root = false;
    let mut seen_doctype = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => {
                if depth == 0 {
                    if root_element.is_some() {
                        return Err(invalid_document(
                            path,
                            reader.error_position(),
                            "document has more than one root element",
                        ));
                    }
                    root_element =
                        Some(String::from_utf8_lossy(event.name().as_ref()).into_owned());
                    seen_before_root = true;
                }
                depth += 1;
            }
            Ok(Event::Empty(event)) => {
                if depth == 0 {
                    if root_element.is_some() {
                        return Err(invalid_document(
                            path,
                            reader.error_position(),
                            "document has more than one root element",
                        ));
                    }
                    root_element =
                        Some(String::from_utf8_lossy(event.name().as_ref()).into_owned());
                    seen_before_root = true;
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Text(event)) => {
                if depth == 0 && has_non_xml_whitespace(event.as_ref()) {
                    return Err(invalid_document(
                        path,
                        reader.error_position(),
                        "document has text outside the root element",
                    ));
                }
                if depth == 0 && root_element.is_none() {
                    seen_before_root = true;
                }
            }
            Ok(Event::CData(_) | Event::GeneralRef(_)) if depth == 0 => {
                return Err(invalid_document(
                    path,
                    reader.error_position(),
                    "document has character data outside the root element",
                ));
            }
            Ok(Event::Decl(_)) => {
                if depth > 0 || root_element.is_some() {
                    return Err(invalid_document(
                        path,
                        reader.error_position(),
                        "document declaration appears after the root element",
                    ));
                }
                if seen_before_root {
                    return Err(invalid_document(
                        path,
                        reader.error_position(),
                        "document declaration is not at document start",
                    ));
                }
                seen_before_root = true;
            }
            Ok(Event::DocType(_)) => {
                if depth > 0 || root_element.is_some() {
                    return Err(invalid_document(
                        path,
                        reader.error_position(),
                        "document type appears after the root element",
                    ));
                }
                if seen_doctype {
                    return Err(invalid_document(
                        path,
                        reader.error_position(),
                        "document has more than one document type declaration",
                    ));
                }
                seen_doctype = true;
                seen_before_root = true;
            }
            Ok(Event::Eof) => break,
            Ok(Event::Comment(_) | Event::PI(_)) if depth == 0 && root_element.is_none() => {
                seen_before_root = true;
            }
            Ok(_) => {}
            Err(source) => {
                return Err(XmlEdtError::Parse {
                    path: path.to_path_buf(),
                    position: reader.error_position(),
                    source,
                });
            }
        }
    }

    root_element.ok_or_else(|| XmlEdtError::MissingRoot {
        path: path.to_path_buf(),
    })
}

fn invalid_document(path: &Path, position: u64, message: &str) -> XmlEdtError {
    XmlEdtError::InvalidDocument {
        path: path.to_path_buf(),
        position,
        message: message.to_owned(),
    }
}

fn has_non_xml_whitespace(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
}
