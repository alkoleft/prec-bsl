use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::xml_edt::parse_document;
use prec_bsl_pipeline::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use prec_bsl_source::{SourceFileKind, SourceRoot};

pub const METADATA_SYNC_RULE: &str = "СинхронизацияОбъектовМетаданныхИФайлов";

const CONFIGURATION_DIR: &str = "Configuration";
const EDT_CONFIGURATION_FILE: &str = "Configuration.mdo";
const DESIGNER_CONFIGURATION_FILE: &str = "Configuration.xml";
const SEQUENCE_TYPE: &str = "Sequence";

pub fn metadata_sync(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    let Some(target) = SyncTarget::from_context(context) else {
        return ScenarioRun::single(ScenarioResult::skipped(
            context.rule_id,
            context.file.repo_path.clone(),
            "scenario handles only configuration and metadata object description files",
        ));
    };

    let issues = match check_configuration(context.repo_root, &context.file.source_root, &target) {
        Ok(issues) => issues,
        Err(message) => {
            return ScenarioRun::single(ScenarioResult::hard_failure(
                context.rule_id,
                target.config_path,
                message,
            ));
        }
    };

    ScenarioRun {
        results: issues
            .into_iter()
            .map(|issue| ScenarioResult::hard_failure(context.rule_id, issue.path, issue.message))
            .collect(),
        post_processing_paths: Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataSyncCheck {
    Clean,
    Failed(Vec<MetadataSyncIssue>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataSyncIssue {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetadataFormat {
    Edt,
    Designer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SyncTarget {
    format: MetadataFormat,
    config_path: PathBuf,
}

impl SyncTarget {
    fn from_context(context: &ScenarioExecutionContext<'_>) -> Option<Self> {
        if is_edt_configuration_path(&context.file.repo_path) {
            return Some(Self {
                format: MetadataFormat::Edt,
                config_path: context.file.repo_path.clone(),
            });
        }
        if is_designer_configuration_path(&context.file.repo_path) {
            return Some(Self {
                format: MetadataFormat::Designer,
                config_path: context.file.repo_path.clone(),
            });
        }

        if context.file.staged_status.is_none() {
            return None;
        }

        match context.file.kind {
            SourceFileKind::EdtMetadata if is_edt_object_description(&context.file.repo_path) => {
                Some(Self {
                    format: MetadataFormat::Edt,
                    config_path: edt_configuration_path(&context.file.source_root),
                })
            }
            SourceFileKind::XmlMetadata
                if is_designer_object_description(&context.file.repo_path) =>
            {
                Some(Self {
                    format: MetadataFormat::Designer,
                    config_path: designer_configuration_path(&context.file.source_root),
                })
            }
            _ => None,
        }
    }
}

pub fn check_metadata_sync_text(
    repo_root: &Path,
    source_root: &SourceRoot,
    config_path: &Path,
    kind: SourceFileKind,
    input: &str,
) -> MetadataSyncCheck {
    let format = if is_edt_configuration_path(config_path) {
        MetadataFormat::Edt
    } else if is_designer_configuration_path(config_path) {
        MetadataFormat::Designer
    } else {
        return MetadataSyncCheck::Failed(vec![MetadataSyncIssue {
            path: config_path.to_path_buf(),
            message: "metadata sync requires Configuration.mdo or Configuration.xml".to_owned(),
        }]);
    };

    if let Err(error) = parse_document(config_path, kind, input) {
        return MetadataSyncCheck::Failed(vec![MetadataSyncIssue {
            path: config_path.to_path_buf(),
            message: error.to_string(),
        }]);
    }

    let references = match parse_metadata_references(config_path, format, input) {
        Ok(references) => references,
        Err(message) => {
            return MetadataSyncCheck::Failed(vec![MetadataSyncIssue {
                path: config_path.to_path_buf(),
                message,
            }]);
        }
    };

    let issues = collect_filesystem_issues(repo_root, source_root, format, &references);
    if issues.is_empty() {
        MetadataSyncCheck::Clean
    } else {
        MetadataSyncCheck::Failed(issues)
    }
}

fn check_configuration(
    repo_root: &Path,
    source_root: &SourceRoot,
    target: &SyncTarget,
) -> Result<Vec<MetadataSyncIssue>, String> {
    let absolute_path = repo_root.join(&target.config_path);
    let input = fs::read_to_string(&absolute_path).map_err(|error| {
        format!(
            "failed to read configuration description {}: {error}",
            target.config_path.display()
        )
    })?;
    let kind = match target.format {
        MetadataFormat::Edt => SourceFileKind::ConfigurationMetadata,
        MetadataFormat::Designer => SourceFileKind::XmlMetadata,
    };

    match check_metadata_sync_text(repo_root, source_root, &target.config_path, kind, &input) {
        MetadataSyncCheck::Clean => Ok(Vec::new()),
        MetadataSyncCheck::Failed(issues) => Ok(issues),
    }
}

fn parse_metadata_references(
    path: &Path,
    format: MetadataFormat,
    input: &str,
) -> Result<BTreeMap<&'static MetadataType, BTreeSet<String>>, String> {
    let mut reader = Reader::from_str(input);
    reader.config_mut().trim_text(false);
    let mut stack = Vec::<String>::new();
    let mut references = BTreeMap::<&'static MetadataType, BTreeSet<String>>::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => stack.push(local_name(event.name().as_ref())),
            Ok(Event::Empty(_event)) => {}
            Ok(Event::End(_event)) => {
                stack.pop();
            }
            Ok(Event::Text(event)) => {
                let text = event
                    .decode()
                    .map_err(|error| format!("failed to decode metadata reference: {error}"))?;
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match format {
                    MetadataFormat::Edt => {
                        if stack.len() != 2 {
                            continue;
                        }
                        let Some(metadata_type) = metadata_type_by_edt_tag(&stack[1]) else {
                            continue;
                        };
                        let Some((type_name, object_name)) = trimmed.split_once('.') else {
                            continue;
                        };
                        if type_name == metadata_type.type_name && !object_name.is_empty() {
                            references
                                .entry(metadata_type)
                                .or_default()
                                .insert(object_name.to_owned());
                        }
                    }
                    MetadataFormat::Designer => {
                        if stack.len() != 4
                            || stack[0] != "MetaDataObject"
                            || stack[1] != "Configuration"
                            || stack[2] != "ChildObjects"
                        {
                            continue;
                        }
                        let Some(metadata_type) = metadata_type_by_type_name(
                            stack.last().expect("stack length was checked"),
                        ) else {
                            continue;
                        };
                        references
                            .entry(metadata_type)
                            .or_default()
                            .insert(trimmed.to_owned());
                    }
                }
            }
            Ok(Event::CData(_event)) => {}
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(format!(
                    "failed to parse metadata references in {} at byte {}: {error}",
                    path.display(),
                    reader.error_position()
                ));
            }
        }
    }

    Ok(references)
}

fn collect_filesystem_issues(
    repo_root: &Path,
    source_root: &SourceRoot,
    format: MetadataFormat,
    references: &BTreeMap<&'static MetadataType, BTreeSet<String>>,
) -> Vec<MetadataSyncIssue> {
    let mut issues = Vec::new();
    for metadata_type in METADATA_TYPES {
        let expected = references.get(metadata_type);
        if metadata_type.type_name == SEQUENCE_TYPE {
            continue;
        }

        let type_dir = source_root.repo_relative_path.join(metadata_type.directory);
        let absolute_type_dir = repo_root.join(&type_dir);
        let expected_names = expected.cloned().unwrap_or_default();
        if !absolute_type_dir.is_dir() {
            if !expected_names.is_empty() {
                issues.push(MetadataSyncIssue {
                    path: type_dir,
                    message: format!(
                        "missing metadata directory for {} objects",
                        metadata_type.type_name
                    ),
                });
            }
            continue;
        }

        match format {
            MetadataFormat::Edt => {
                if let Err(issue) = collect_edt_type_issues(
                    repo_root,
                    &type_dir,
                    metadata_type,
                    &expected_names,
                    &mut issues,
                ) {
                    issues.push(issue);
                }
            }
            MetadataFormat::Designer => {
                if let Err(issue) = collect_designer_type_issues(
                    repo_root,
                    &type_dir,
                    metadata_type,
                    &expected_names,
                    &mut issues,
                ) {
                    issues.push(issue);
                }
            }
        }
    }
    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.message.cmp(&right.message))
    });
    issues
}

fn collect_edt_type_issues(
    repo_root: &Path,
    type_dir: &Path,
    metadata_type: &MetadataType,
    expected_names: &BTreeSet<String>,
    issues: &mut Vec<MetadataSyncIssue>,
) -> Result<(), MetadataSyncIssue> {
    let entries = read_dir_entries(repo_root, type_dir)?;
    let actual_dirs = entries
        .iter()
        .filter(|entry| entry.is_dir)
        .map(|entry| entry.name.clone())
        .collect::<BTreeSet<_>>();

    for expected_name in expected_names {
        let expected_dir = type_dir.join(expected_name);
        match find_case_mismatch(&actual_dirs, expected_name) {
            Some(actual_name) => {
                issues.push(MetadataSyncIssue {
                    path: type_dir.join(actual_name),
                    message: format!(
                        "metadata object directory name differs by case: expected {}.{}",
                        metadata_type.type_name, expected_name
                    ),
                });
                continue;
            }
            None if !repo_root.join(&expected_dir).is_dir() => {
                issues.push(MetadataSyncIssue {
                    path: expected_dir,
                    message: format!(
                        "missing files for {}.{}",
                        metadata_type.type_name, expected_name
                    ),
                });
                continue;
            }
            None => {}
        }

        let expected_file_name = format!("{expected_name}.mdo");
        let object_entries = read_dir_entries(repo_root, &expected_dir)?;
        let actual_files = object_entries
            .iter()
            .filter(|entry| entry.is_file)
            .map(|entry| entry.name.clone())
            .collect::<BTreeSet<_>>();
        match find_case_mismatch(&actual_files, &expected_file_name) {
            Some(actual_name) => issues.push(MetadataSyncIssue {
                path: expected_dir.join(actual_name),
                message: format!(
                    "metadata object file name differs by case: expected {}",
                    expected_file_name
                ),
            }),
            None if !repo_root
                .join(expected_dir.join(&expected_file_name))
                .is_file() =>
            {
                issues.push(MetadataSyncIssue {
                    path: expected_dir.join(expected_file_name),
                    message: format!(
                        "missing files for {}.{}",
                        metadata_type.type_name, expected_name
                    ),
                });
            }
            None => {}
        }
    }

    for entry in entries {
        if entry.is_dir && !expected_names.contains(&entry.name) {
            if find_normalized(expected_names, &entry.name).is_some() {
                continue;
            }
            issues.push(MetadataSyncIssue {
                path: type_dir.join(&entry.name),
                message: format!(
                    "unreferenced metadata object directory for {}: {}",
                    metadata_type.type_name, entry.name
                ),
            });
            for object_file in read_dir_entries(repo_root, &type_dir.join(&entry.name))?
                .into_iter()
                .filter(|object_file| {
                    object_file.is_file
                        && Path::new(&object_file.name)
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("mdo"))
                })
            {
                issues.push(MetadataSyncIssue {
                    path: type_dir.join(&entry.name).join(&object_file.name),
                    message: format!(
                        "unreferenced metadata object file for {}: {}",
                        metadata_type.type_name, object_file.name
                    ),
                });
            }
        } else if entry.is_file
            && Path::new(&entry.name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mdo"))
        {
            let object_name = Path::new(&entry.name)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !expected_names.contains(object_name) {
                issues.push(MetadataSyncIssue {
                    path: type_dir.join(&entry.name),
                    message: format!(
                        "unreferenced metadata object file for {}: {}",
                        metadata_type.type_name, entry.name
                    ),
                });
            }
        }
    }
    Ok(())
}

fn collect_designer_type_issues(
    repo_root: &Path,
    type_dir: &Path,
    metadata_type: &MetadataType,
    expected_names: &BTreeSet<String>,
    issues: &mut Vec<MetadataSyncIssue>,
) -> Result<(), MetadataSyncIssue> {
    let entries = read_dir_entries(repo_root, type_dir)?;
    let actual_files = entries
        .iter()
        .filter(|entry| entry.is_file)
        .map(|entry| entry.name.clone())
        .collect::<BTreeSet<_>>();

    for expected_name in expected_names {
        let expected_file_name = format!("{expected_name}.xml");
        match find_case_mismatch(&actual_files, &expected_file_name) {
            Some(actual_name) => issues.push(MetadataSyncIssue {
                path: type_dir.join(actual_name),
                message: format!(
                    "metadata object file name differs by case: expected {}",
                    expected_file_name
                ),
            }),
            None if !repo_root.join(type_dir.join(&expected_file_name)).is_file() => {
                issues.push(MetadataSyncIssue {
                    path: type_dir.join(expected_file_name),
                    message: format!(
                        "missing files for {}.{}",
                        metadata_type.type_name, expected_name
                    ),
                });
            }
            None => {}
        }
    }

    for entry in entries {
        if entry.is_file
            && Path::new(&entry.name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        {
            let object_name = Path::new(&entry.name)
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if !expected_names.contains(object_name) {
                issues.push(MetadataSyncIssue {
                    path: type_dir.join(&entry.name),
                    message: format!(
                        "unreferenced metadata object file for {}: {}",
                        metadata_type.type_name, entry.name
                    ),
                });
            }
        } else if entry.is_dir && !expected_names.contains(&entry.name) {
            issues.push(MetadataSyncIssue {
                path: type_dir.join(&entry.name),
                message: format!(
                    "unreferenced metadata object directory for {}: {}",
                    metadata_type.type_name, entry.name
                ),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirectoryEntry {
    name: String,
    is_dir: bool,
    is_file: bool,
}

fn read_dir_entries(
    repo_root: &Path,
    repo_path: &Path,
) -> Result<Vec<DirectoryEntry>, MetadataSyncIssue> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(repo_root.join(repo_path)).map_err(|error| MetadataSyncIssue {
        path: repo_path.to_path_buf(),
        message: format!("failed to read metadata directory: {error}"),
    })?;
    for entry in read_dir {
        let entry = entry.map_err(|error| MetadataSyncIssue {
            path: repo_path.to_path_buf(),
            message: format!("failed to read metadata directory entry: {error}"),
        })?;
        let file_type = entry.file_type().map_err(|error| MetadataSyncIssue {
            path: repo_path.join(entry.file_name()),
            message: format!("failed to read metadata directory entry type: {error}"),
        })?;
        entries.push(DirectoryEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            is_dir: file_type.is_dir(),
            is_file: file_type.is_file(),
        });
    }
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn find_case_mismatch<'a>(actual_names: &'a BTreeSet<String>, expected: &str) -> Option<&'a str> {
    let normalized_expected = normalize_name(expected);
    actual_names
        .iter()
        .find(|actual| actual.as_str() != expected && normalize_name(actual) == normalized_expected)
        .map(String::as_str)
}

fn find_normalized<'a>(values: &'a BTreeSet<String>, value: &str) -> Option<&'a str> {
    let normalized_value = normalize_name(value);
    values
        .iter()
        .find(|actual| normalize_name(actual) == normalized_value)
        .map(String::as_str)
}

fn normalize_name(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MetadataType {
    directory: &'static str,
    type_name: &'static str,
    edt_tag: &'static str,
}

const METADATA_TYPES: &[MetadataType] = &[
    MetadataType::new(
        "AccountingRegisters",
        "AccountingRegister",
        "accountingRegisters",
    ),
    MetadataType::new(
        "AccumulationRegisters",
        "AccumulationRegister",
        "accumulationRegisters",
    ),
    MetadataType::new("Bots", "Bot", "bots"),
    MetadataType::new("BusinessProcesses", "BusinessProcess", "businessProcesses"),
    MetadataType::new("Catalogs", "Catalog", "catalogs"),
    MetadataType::new("ChartsOfAccounts", "ChartOfAccounts", "chartsOfAccounts"),
    MetadataType::new(
        "ChartsOfCalculationTypes",
        "ChartOfCalculationTypes",
        "chartsOfCalculationTypes",
    ),
    MetadataType::new(
        "ChartsOfCharacteristicTypes",
        "ChartOfCharacteristicTypes",
        "chartsOfCharacteristicTypes",
    ),
    MetadataType::new(
        "CalculationRegisters",
        "CalculationRegister",
        "calculationRegisters",
    ),
    MetadataType::new("CommandGroups", "CommandGroup", "commandGroups"),
    MetadataType::new("CommonAttributes", "CommonAttribute", "commonAttributes"),
    MetadataType::new("CommonCommands", "CommonCommand", "commonCommands"),
    MetadataType::new("CommonForms", "CommonForm", "commonForms"),
    MetadataType::new("CommonModules", "CommonModule", "commonModules"),
    MetadataType::new("CommonPictures", "CommonPicture", "commonPictures"),
    MetadataType::new("CommonTemplates", "CommonTemplate", "commonTemplates"),
    MetadataType::new("Constants", "Constant", "constants"),
    MetadataType::new("DataProcessors", "DataProcessor", "dataProcessors"),
    MetadataType::new("DefinedTypes", "DefinedType", "definedTypes"),
    MetadataType::new("DocumentJournals", "DocumentJournal", "documentJournals"),
    MetadataType::new(
        "DocumentNumerators",
        "DocumentNumerator",
        "documentNumerators",
    ),
    MetadataType::new("Documents", "Document", "documents"),
    MetadataType::new("Enums", "Enum", "enums"),
    MetadataType::new(
        "EventSubscriptions",
        "EventSubscription",
        "eventSubscriptions",
    ),
    MetadataType::new("ExchangePlans", "ExchangePlan", "exchangePlans"),
    MetadataType::new(
        "ExternalDataSources",
        "ExternalDataSource",
        "externalDataSources",
    ),
    MetadataType::new("FilterCriteria", "FilterCriterion", "filterCriteria"),
    MetadataType::new("FunctionalOptions", "FunctionalOption", "functionalOptions"),
    MetadataType::new(
        "FunctionalOptionsParameters",
        "FunctionalOptionsParameter",
        "functionalOptionsParameters",
    ),
    MetadataType::new("HTTPServices", "HTTPService", "httpServices"),
    MetadataType::new(
        "InformationRegisters",
        "InformationRegister",
        "informationRegisters",
    ),
    MetadataType::new(
        "IntegrationServices",
        "IntegrationService",
        "integrationServices",
    ),
    MetadataType::new("Interfaces", "Interface", "interfaces"),
    MetadataType::new("Languages", "Language", "languages"),
    MetadataType::new("Reports", "Report", "reports"),
    MetadataType::new("Roles", "Role", "roles"),
    MetadataType::new("ScheduledJobs", "ScheduledJob", "scheduledJobs"),
    MetadataType::new("SessionParameters", "SessionParameter", "sessionParameters"),
    MetadataType::new("SettingsStorages", "SettingsStorage", "settingsStorages"),
    MetadataType::new("StyleItems", "StyleItem", "styleItems"),
    MetadataType::new("Sequences", SEQUENCE_TYPE, "sequences"),
    MetadataType::new("Styles", "Style", "styles"),
    MetadataType::new("Subsystems", "Subsystem", "subsystems"),
    MetadataType::new("Tasks", "Task", "tasks"),
    MetadataType::new("WebServices", "WebService", "webServices"),
    MetadataType::new("WSReferences", "WSReference", "wsReferences"),
    MetadataType::new("XDTOPackages", "XDTOPackage", "xdtoPackages"),
];

impl MetadataType {
    const fn new(directory: &'static str, type_name: &'static str, edt_tag: &'static str) -> Self {
        Self {
            directory,
            type_name,
            edt_tag,
        }
    }
}

fn metadata_type_by_edt_tag(tag: &str) -> Option<&'static MetadataType> {
    METADATA_TYPES
        .iter()
        .find(|metadata_type| metadata_type.edt_tag == tag)
}

fn metadata_type_by_type_name(type_name: &str) -> Option<&'static MetadataType> {
    METADATA_TYPES
        .iter()
        .find(|metadata_type| metadata_type.type_name == type_name)
}

fn is_edt_configuration_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|file_name| file_name == EDT_CONFIGURATION_FILE)
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|parent| parent == CONFIGURATION_DIR)
}

fn is_designer_configuration_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|file_name| file_name == DESIGNER_CONFIGURATION_FILE)
}

fn is_edt_object_description(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mdo"))
        && path.file_stem() == path.parent().and_then(Path::file_name)
        && path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|directory| directory.to_str())
            .is_some_and(|directory| metadata_type_by_directory(directory).is_some())
}

fn is_designer_object_description(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        && !is_designer_configuration_path(path)
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|directory| directory.to_str())
            .is_some_and(|directory| metadata_type_by_directory(directory).is_some())
}

fn metadata_type_by_directory(directory: &str) -> Option<&'static MetadataType> {
    METADATA_TYPES
        .iter()
        .find(|metadata_type| metadata_type.directory == directory)
}

fn edt_configuration_path(source_root: &SourceRoot) -> PathBuf {
    source_root
        .repo_relative_path
        .join(CONFIGURATION_DIR)
        .join(EDT_CONFIGURATION_FILE)
}

fn designer_configuration_path(source_root: &SourceRoot) -> PathBuf {
    source_root
        .repo_relative_path
        .join(DESIGNER_CONFIGURATION_FILE)
}

fn local_name(name: &[u8]) -> String {
    let name = name
        .iter()
        .rposition(|byte| *byte == b':')
        .map(|index| &name[index + 1..])
        .unwrap_or(name);
    String::from_utf8_lossy(name).into_owned()
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn metadata_sync_reports_unreadable_metadata_directory() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .join("metadata-sync-unit-tests")
            .join(format!("{}_unreadable_{}", std::process::id(), nonce));
        let metadata_dir = repo.join("src/CommonModules");
        fs::create_dir_all(&metadata_dir).unwrap();
        let original_permissions = fs::metadata(&metadata_dir).unwrap().permissions();
        fs::set_permissions(&metadata_dir, fs::Permissions::from_mode(0)).unwrap();

        let result = super::read_dir_entries(&repo, std::path::Path::new("src/CommonModules"));

        fs::set_permissions(&metadata_dir, original_permissions).unwrap();
        if result.is_ok() {
            return;
        }
        let issue = result.unwrap_err();
        assert_eq!(issue.path, std::path::PathBuf::from("src/CommonModules"));
        assert!(issue.message.contains("failed to read metadata directory"));
    }
}
