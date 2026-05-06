use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureSeverity {
    Modified,
    Warning,
}

impl fmt::Display for FixtureSeverity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Modified => "modified",
            Self::Warning => "warning",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureDiagnostic {
    pub severity: FixtureSeverity,
    pub rule_id: String,
    pub path: PathBuf,
    pub message: String,
}

impl FixtureDiagnostic {
    pub fn new(
        severity: FixtureSeverity,
        rule_id: impl Into<String>,
        path: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule_id: rule_id.into(),
            path: path.into(),
            message: message.into(),
        }
    }

    fn to_fixture_line(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            self.severity,
            self.rule_id,
            self.path.display(),
            self.message
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureRun {
    pub output: String,
    pub diagnostics: Vec<FixtureDiagnostic>,
}

impl FixtureRun {
    pub fn new(output: impl Into<String>, diagnostics: Vec<FixtureDiagnostic>) -> Self {
        Self {
            output: output.into(),
            diagnostics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoldenFixture {
    root: PathBuf,
    logical_path: PathBuf,
}

impl GoldenFixture {
    pub fn new(root: impl Into<PathBuf>, logical_path: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            logical_path: logical_path.into(),
        }
    }

    pub fn logical_path(&self) -> &Path {
        &self.logical_path
    }

    fn read_text(&self, file_name: &str) -> String {
        let path = self.root.join(file_name);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read fixture {}: {error}", path.display()));

        decode_visible_whitespace(&content)
    }
}

pub fn assert_fixer_fixture<F>(fixture: &GoldenFixture, run: F)
where
    F: Fn(&str, &Path) -> FixtureRun,
{
    let input = fixture.read_text("input.bsl");
    let expected_output = fixture.read_text("expected.bsl");
    let expected_diagnostics = fixture.read_text("diagnostics.txt");

    let first_run = run(&input, fixture.logical_path());
    assert_eq!(
        first_run.output, expected_output,
        "fixer output must match the golden fixture"
    );
    assert_eq!(
        serialize_diagnostics(&first_run.diagnostics),
        expected_diagnostics,
        "fixer diagnostics must match the golden fixture"
    );

    let second_run = run(&first_run.output, fixture.logical_path());
    assert_eq!(
        second_run.output, expected_output,
        "fixer second run must keep the first-run output unchanged"
    );
    assert!(
        second_run.diagnostics.is_empty(),
        "fixer second run must be clean, got: {:?}",
        second_run.diagnostics
    );
}

pub fn assert_checker_fixture<F>(fixture: &GoldenFixture, run: F)
where
    F: Fn(&str, &Path) -> FixtureRun,
{
    let input = fixture.read_text("input.bsl");
    let expected_diagnostics = fixture.read_text("diagnostics.txt");

    let check_run = run(&input, fixture.logical_path());
    assert_eq!(
        check_run.output, input,
        "checker fixtures must not modify input text"
    );
    assert_eq!(
        serialize_diagnostics(&check_run.diagnostics),
        expected_diagnostics,
        "checker diagnostics must match the golden fixture"
    );
}

fn serialize_diagnostics(diagnostics: &[FixtureDiagnostic]) -> String {
    let mut output = diagnostics
        .iter()
        .map(FixtureDiagnostic::to_fixture_line)
        .collect::<Vec<_>>()
        .join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

fn decode_visible_whitespace(content: &str) -> String {
    content.replace("<SP>", " ").replace("<TAB>", "\t")
}
