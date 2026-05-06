mod support;

use std::path::{Path, PathBuf};

use support::fixtures::{
    FixtureDiagnostic, FixtureRun, FixtureSeverity, GoldenFixture, assert_checker_fixture,
    assert_fixer_fixture,
};

const TRAILING_WHITESPACE_RULE: &str = "УдалениеЛишнихКонцевыхПробелов";
const PROFANITY_RULE: &str = "ПроверкаНецензурныхСлов";

#[test]
fn fixtures_support_cyrillic_fixer_output_diagnostics_and_idempotence() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/УдалениеЛишнихКонцевыхПробелов/кириллический_модуль",
        PathBuf::from("src/ОбщиеМодули/КлиентскийМодуль/Модуль.bsl"),
    );

    assert_fixer_fixture(&fixture, sample_trailing_whitespace_fixer);
}

#[test]
fn fixtures_support_checker_diagnostics_without_modifying_input() {
    let fixture = GoldenFixture::new(
        "tests/fixtures/golden/ПроверкаНецензурныхСлов/кириллический_модуль",
        PathBuf::from("src/ОбщиеМодули/СерверныйМодуль/Модуль.bsl"),
    );

    assert_checker_fixture(&fixture, sample_profanity_checker);
}

fn sample_trailing_whitespace_fixer(input: &str, path: &Path) -> FixtureRun {
    let output = input
        .split_inclusive('\n')
        .map(|line| {
            let (body, ending) = line
                .strip_suffix('\n')
                .map(|body| (body, "\n"))
                .unwrap_or((line, ""));
            format!("{}{}", body.trim_end_matches([' ', '\t']), ending)
        })
        .collect::<String>();

    let diagnostics = if output == input {
        Vec::new()
    } else {
        vec![FixtureDiagnostic::new(
            FixtureSeverity::Modified,
            TRAILING_WHITESPACE_RULE,
            path,
            "removed trailing spaces or tabs",
        )]
    };

    FixtureRun::new(output, diagnostics)
}

fn sample_profanity_checker(input: &str, path: &Path) -> FixtureRun {
    let diagnostics = input
        .lines()
        .enumerate()
        .filter(|(_, line)| line.contains("плохоеСлово"))
        .map(|(index, _)| {
            FixtureDiagnostic::new(
                FixtureSeverity::Warning,
                PROFANITY_RULE,
                path,
                format!("matched dictionary word at line {}", index + 1),
            )
        })
        .collect::<Vec<_>>();

    FixtureRun::new(input.to_owned(), diagnostics)
}
