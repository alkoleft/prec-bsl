use std::path::{Path, PathBuf};

pub(crate) fn resolve_repo_path(repo_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    }
}

fn looks_absolute_path(path: &str) -> bool {
    Path::new(path).is_absolute()
        || path.starts_with('\\')
        || path
            .as_bytes()
            .get(1)
            .is_some_and(|character| *character == b':')
}

pub(crate) fn is_repository_relative_path(path: &str) -> bool {
    !looks_absolute_path(path)
        && !path
            .replace('\\', "/")
            .split('/')
            .any(|component| component == "..")
}

pub(crate) fn normalize_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn path_matches_project(source_path: &str, project_path: &str) -> bool {
    source_path == project_path
        || source_path
            .strip_prefix(project_path)
            .is_some_and(|rest| rest.starts_with('/'))
}

pub(crate) fn empty_string_as_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

pub(crate) fn normalize_project_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_matches('/')
        .trim_start_matches("./")
        .to_owned()
}
