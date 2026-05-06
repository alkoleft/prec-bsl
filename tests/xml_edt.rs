use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use prec_bsl::source_files::{SourceFileKind, classify_repo_path, resolve_source_roots};
use prec_bsl::xml_edt::{
    XmlEdtError, parse_document, read_document, write_document, write_validated_xml,
};

#[test]
fn xml_edt_reads_supported_metadata_file_kinds() {
    let repo = temp_repo("supported_kinds");
    write_file(
        repo.join("src/Configuration.mdo"),
        r#"<?xml version="1.0" encoding="UTF-8"?><mdclass:Configuration/>"#,
    );
    write_file(
        repo.join("src/Catalogs/Товары/Object.mdo"),
        r#"<mdclass:Catalog uuid="1"><name>Товары</name></mdclass:Catalog>"#,
    );
    write_file(
        repo.join("src/Catalogs/Товары/Forms/Форма.form"),
        r#"<form:Form><items/></form:Form>"#,
    );
    write_file(
        repo.join("src/Designer/ConfigDumpInfo.xml"),
        r#"<ConfigDumpInfo><ConfigVersions/></ConfigDumpInfo>"#,
    );
    let roots = resolve_source_roots(&repo, &[PathBuf::from("src")]).roots;

    let cases = [
        (
            "src/Configuration.mdo",
            SourceFileKind::ConfigurationMetadata,
            "mdclass:Configuration",
        ),
        (
            "src/Catalogs/Товары/Object.mdo",
            SourceFileKind::EdtMetadata,
            "mdclass:Catalog",
        ),
        (
            "src/Catalogs/Товары/Forms/Форма.form",
            SourceFileKind::EdtForm,
            "form:Form",
        ),
        (
            "src/Designer/ConfigDumpInfo.xml",
            SourceFileKind::XmlMetadata,
            "ConfigDumpInfo",
        ),
    ];

    for (repo_path, expected_kind, expected_root) in cases {
        let file = classify_repo_path(&roots, repo_path, None).unwrap();
        let document = read_document(&repo, &file).unwrap();

        assert_eq!(document.kind, expected_kind);
        assert_eq!(document.path, PathBuf::from(repo_path));
        assert_eq!(document.root_element, expected_root);
    }
}

#[test]
fn xml_edt_rejects_unsupported_file_kinds() {
    let error = parse_document(
        PathBuf::from("src/Модуль.bsl"),
        SourceFileKind::BslModule,
        "Процедура Тест()\nКонецПроцедуры\n",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        XmlEdtError::UnsupportedKind {
            ref path,
            kind: SourceFileKind::BslModule,
        } if path == Path::new("src/Модуль.bsl")
    ));
}

#[test]
fn xml_edt_writer_preserves_existing_text_layout_for_clean_roundtrip() {
    let input = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<mdclass:Catalog uuid=\"1\">\n",
        "  <name>Товары</name>\n",
        "  <description>Каталог товаров</description>\n",
        "</mdclass:Catalog>\n",
    );
    let document = parse_document(
        PathBuf::from("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        input,
    )
    .unwrap();

    let output = write_document(&document).unwrap();

    assert_eq!(output, input);
}

#[test]
fn xml_edt_reports_parse_errors_with_repo_path_and_position() {
    let error = parse_document(
        PathBuf::from("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        "<mdclass:Catalog><name>Товары</mdclass:Catalog>",
    )
    .unwrap_err();

    match error {
        XmlEdtError::Parse { path, position, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert!(
                position > 0,
                "parse error must include a useful byte position"
            );
        }
        other => panic!("expected parse error, got {other:?}"),
    }
}

#[test]
fn xml_edt_reports_empty_documents_as_missing_root() {
    let error = parse_document(
        PathBuf::from("src/empty.form"),
        SourceFileKind::EdtForm,
        "\n  \n",
    )
    .unwrap_err();

    assert!(matches!(
        error,
        XmlEdtError::MissingRoot { ref path } if path == Path::new("src/empty.form")
    ));
}

#[test]
fn xml_edt_rejects_multiple_root_elements_with_repo_path() {
    let error = parse_document(
        PathBuf::from("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        "<mdclass:Catalog/><mdclass:Catalog/>",
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(message, "document has more than one root element");
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

#[test]
fn xml_edt_rejects_trailing_text_outside_root_at_writer_boundary() {
    let error = write_validated_xml(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        "<mdclass:Catalog></mdclass:Catalog>tail",
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(message, "document has text outside the root element");
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

#[test]
fn xml_edt_rejects_declaration_after_root_at_writer_boundary() {
    let error = write_validated_xml(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        r#"<mdclass:Catalog/><?xml version="1.0"?>"#,
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(
                message,
                "document declaration appears after the root element"
            );
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

#[test]
fn xml_edt_rejects_doctype_after_root_with_repo_path() {
    let error = parse_document(
        PathBuf::from("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        "<mdclass:Catalog/><!DOCTYPE mdclass:Catalog>",
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(message, "document type appears after the root element");
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

#[test]
fn xml_edt_rejects_declaration_after_comment_before_root() {
    let error = parse_document(
        PathBuf::from("src/Catalogs/Товары/Object.mdo"),
        SourceFileKind::EdtMetadata,
        r#"<!--metadata--><?xml version="1.0"?><mdclass:Catalog/>"#,
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(message, "document declaration is not at document start");
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

#[test]
fn xml_edt_rejects_duplicate_doctype_before_root() {
    let error = write_validated_xml(
        Path::new("src/Catalogs/Товары/Object.mdo"),
        "<!DOCTYPE mdclass:Catalog><!DOCTYPE mdclass:Catalog><mdclass:Catalog/>",
    )
    .unwrap_err();

    match error {
        XmlEdtError::InvalidDocument { path, message, .. } => {
            assert_eq!(path, PathBuf::from("src/Catalogs/Товары/Object.mdo"));
            assert_eq!(
                message,
                "document has more than one document type declaration"
            );
        }
        other => panic!("expected invalid document error, got {other:?}"),
    }
}

fn temp_repo(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time must be after UNIX_EPOCH")
        .as_nanos();
    let path = std::env::current_dir()
        .expect("current dir must be available")
        .join("target")
        .join("xml-edt-tests")
        .join(format!("{}_{}_{}", std::process::id(), test_name, nonce));
    fs::create_dir_all(&path).expect("temporary XML/EDT test repo must be created");
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory must be created");
    }
    fs::write(path, content).expect("test fixture must be written");
}
