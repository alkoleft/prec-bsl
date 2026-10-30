use std::path::PathBuf;

use serde_json::Value;

use super::*;

#[test]
fn line_diff_matches_naive_lcs_on_small_inputs() {
    // Reference cases that exercise every branch of the Hirschberg base
    // cases and splits.
    let cases: &[(&str, &str, Vec<&str>)] = &[
        // identical content: no lines
        ("a\nb\n", "a\nb\n", vec![]),
        // pure addition
        ("a\n", "a\nb\n", vec!["+b"]),
        // pure removal
        ("a\nb\n", "a\n", vec!["-b"]),
        // modification of a middle line
        ("a\nb\nc\n", "a\nB\nc\n", vec!["-b", "+B"]),
        // single original line matched later
        ("x\n", "a\nx\n", vec!["+a"]),
        // single proposed line matched earlier
        ("a\nb\nx\n", "x\n", vec!["-a", "-b"]),
        // full rewrite
        ("a\nb\n", "c\nd\n", vec!["-a", "-b", "+c", "+d"]),
        // empty sides
        ("", "a\n", vec!["+a"]),
        ("a\n", "", vec!["-a"]),
        ("", "", vec![]),
        // interleaved: repeated lines keep the LCS alignment stable
        ("a\nb\na\nb\n", "b\na\nb\n", vec!["-a"]),
    ];

    for (original, proposed, expected) in cases {
        assert_eq!(
            render_line_diff(original, proposed),
            *expected,
            "diff of {original:?} -> {proposed:?}"
        );
    }
}

#[test]
fn line_diff_handles_large_files_in_linear_memory() {
    // 20 000 lines on both sides: the quadratic table would be
    // 20 001 * 20 001 usize cells (~3.2 GB); the linear implementation must
    // handle it, and a single-line edit in the middle must stay a small
    // diff instead of rewriting the whole file.
    let line = "Процедура ОдинаковаяСтрока";
    let mut original = String::new();
    for index in 0..20_000 {
        use std::fmt::Write as _;
        writeln!(original, "{line} {index}").unwrap();
    }
    let mut proposed = original.clone();
    let target = line_count_offset(&proposed, 9_999);
    proposed.replace_range(
        target..target + "Процедура ОдинаковаяСтрока 9999".len(),
        "Процедура ИзменённаяСтрока 9999",
    );

    let diff = render_line_diff(&original, &proposed);

    // The LCS diff must contain only the edited line pair, not the whole
    // file: 20 000 unchanged lines are skipped.
    assert_eq!(diff.len(), 2, "expected a 2-line diff, got: {diff:?}");
    assert!(diff.contains(&"-Процедура ОдинаковаяСтрока 9999".to_owned()));
    assert!(diff.contains(&"+Процедура ИзменённаяСтрока 9999".to_owned()));
}

/// Byte offset of the start of the 0-based `target` line in `text`.
fn line_count_offset(text: &str, target: usize) -> usize {
    let mut offset = 0;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if index == target {
            return offset;
        }
        offset += line.len();
    }
    panic!(
        "line {target} not found in text of {} lines",
        text.lines().count()
    );
}

#[test]
fn output_text_groups_messages_and_separates_modified_files_and_failures() {
    let report = sample_report();

    let output = render_text_report(&report);

    assert_eq!(
        output,
        concat!(
            "prec-bsl хук: обработано файлов: 2\n",
            "\n",
            "Измененные файлы:\n",
            "  src/Исправленный.bsl\n",
            "\n",
            "Блокирующие ошибки:\n",
            "  [ПроверкаНецензурныхСлов] src/Плохой.bsl: найдено слово из словаря 'плохоеСлово' в строке 1\n",
            "\n",
            "Сообщения:\n",
            "  УдалениеЛишнихКонцевыхПробелов\n",
            "    src/Исправленный.bsl\n",
            "      изменено: удалены лишние пробелы или табы в конце строк\n",
            "      предупреждение: файл содержит табы\n",
            "\n",
            "Код выхода: 1\n"
        )
    );
}

#[test]
fn output_text_is_stable_for_shuffled_results() {
    let original = sample_report();
    let mut shuffled = original.clone();
    shuffled.results.reverse();

    assert_eq!(render_text_report(&original), render_text_report(&shuffled));
}

#[test]
fn output_text_renders_unsupported_as_blocking_failure() {
    let report = PipelineReport {
        mode: PipelineMode::ExecRules,
        processed_files: vec![PathBuf::from("src/Модуль.bsl")],
        results: vec![ScenarioResult::unsupported(
            "ЛокальныйСценарий",
            "src/Модуль.bsl",
            "dynamic local .os execution is not supported in v1",
        )],
    };

    let output = render_text_report(&report);

    assert!(output.contains("Блокирующие ошибки:\n  [ЛокальныйСценарий] src/Модуль.bsl"));
    assert!(output.contains(
        "src/Модуль.bsl: не поддерживается: динамическое выполнение локальных .os-сценариев не поддерживается в v1"
    ));
    assert!(!output.contains("Сообщения:"));
    assert!(output.contains("Код выхода: 1"));
}

#[test]
fn output_text_suppresses_skipped_messages_when_actionable_results_exist() {
    let report = PipelineReport {
        mode: PipelineMode::Hook,
        processed_files: vec![
            PathBuf::from("src/Форма.form"),
            PathBuf::from("src/Модуль.bsl"),
        ],
        results: vec![
            ScenarioResult::skipped(
                "ВставкаКопирайтов",
                "src/Форма.form",
                "scenario handles only BSL modules",
            ),
            ScenarioResult::hard_failure(
                "КорректировкаXMLФорм",
                "src/Форма.form",
                "form element id must be a positive integer: -1",
            ),
            ScenarioResult::warning(
                "УдалениеЛишнихКонцевыхПробелов",
                "src/Модуль.bsl",
                "file contained tabs",
            ),
        ],
    };

    let output = render_text_report(&report);

    assert!(output.contains("Блокирующие ошибки:"));
    assert!(output.contains("Сообщения:"));
    assert!(output.contains("предупреждение: файл содержит табы"));
    assert!(!output.contains("scenario handles only BSL modules"));
    assert!(!output.contains("ошибка: идентификатор элемента формы"));
}

#[test]
fn output_text_keeps_skipped_messages_when_they_are_the_only_explanation() {
    let report = PipelineReport {
        mode: PipelineMode::Hook,
        processed_files: vec![PathBuf::from("src/Форма.form")],
        results: vec![ScenarioResult::skipped(
            "ВставкаКопирайтов",
            "src/Форма.form",
            "scenario handles only BSL modules",
        )],
    };

    let output = render_text_report(&report);

    assert!(output.contains("Сообщения:"));
    assert!(output.contains("пропущено: сценарий обрабатывает только модули BSL"));
}

#[test]
fn output_text_localizes_real_xml_messages() {
    let report = PipelineReport {
        mode: PipelineMode::ExecRules,
        processed_files: vec![PathBuf::from("src/Form.form")],
        results: vec![ScenarioResult::modified(
            "КорректировкаXMLФорм",
            "src/Form.form",
            "corrected duplicate XML form element ids",
        )],
    };

    let output = render_text_report(&report);

    assert!(output.contains(
        "изменено: исправлены повторяющиеся или неположительные идентификаторы элементов XML-формы"
    ));
    assert!(!output.contains("corrected duplicate XML form element ids"));
}

#[test]
fn output_text_localizes_line_length_messages() {
    let report = PipelineReport {
        mode: PipelineMode::Check,
        processed_files: vec![PathBuf::from("src/Модуль.bsl")],
        results: vec![
            ScenarioResult::warning(
                "ПроверкаДлиныСтрок",
                "src/Модуль.bsl",
                "String length 160 exceeded maximum 150 at line 3",
            ),
            ScenarioResult::hard_failure(
                "ПроверкаДлиныСтрок",
                "src/Модуль.bsl",
                "max line length setting МаксимальнаяДлинаСтроки must be a positive integer",
            ),
        ],
    };

    let output = render_text_report(&report);

    assert!(output.contains(
        "предупреждение: длина строки 160 превышает максимально допустимую 150 (строка 3)"
    ));
    assert!(!output.contains("String length 160"));
    assert!(
        output.contains("настройка МаксимальнаяДлинаСтроки должна быть целым положительным числом")
    );
    assert!(!output.contains("must be a positive integer"));
}

#[test]
fn output_json_contains_stable_contract_fields_and_source_span() {
    let report = sample_report();

    let output = render_json_report(&report);
    let json = serde_json::from_str::<Value>(&output).unwrap();
    let results = json["results"].as_array().unwrap();

    assert_eq!(json["mode"], "hook");
    assert_eq!(json["exit_code"], 1);
    assert_eq!(json["processed_files"][0], "src/Исправленный.bsl");
    assert_eq!(json["modified_files"][0], "src/Исправленный.bsl");

    for result in results {
        for field in ["rule_id", "path", "severity", "modified", "message"] {
            assert!(
                result.get(field).is_some(),
                "missing required JSON result field {field}: {result}"
            );
        }
    }

    let profanity = results
        .iter()
        .find(|result| result["rule_id"] == "ПроверкаНецензурныхСлов")
        .unwrap();
    assert_eq!(profanity["path"], "src/Плохой.bsl");
    assert_eq!(profanity["severity"], "hard_failure");
    assert_eq!(profanity["modified"], false);
    assert_eq!(
        profanity["message"],
        "matched dictionary word 'плохоеСлово' at line 1"
    );
    assert_eq!(profanity["source_span"]["start_byte"], 7);
    assert_eq!(profanity["source_span"]["end_byte"], 13);

    let trailing_whitespace = results
        .iter()
        .find(|result| result["severity"] == "modified")
        .unwrap();
    assert_eq!(
        trailing_whitespace["modified_paths"][0],
        "src/Исправленный.bsl"
    );
}

#[test]
fn render_report_dispatches_by_cli_format() {
    let report = PipelineReport {
        mode: PipelineMode::ExecRules,
        processed_files: Vec::new(),
        results: Vec::new(),
    };

    assert!(render_report(&report, OutputFormat::Text).contains("prec-bsl exec-rules"));
    assert!(render_report(&report, OutputFormat::Json).contains("\"mode\": \"exec-rules\""));
}

fn sample_report() -> PipelineReport {
    PipelineReport {
        mode: PipelineMode::Hook,
        processed_files: vec![
            PathBuf::from("src/Исправленный.bsl"),
            PathBuf::from("src/Плохой.bsl"),
        ],
        results: vec![
            ScenarioResult::warning(
                "УдалениеЛишнихКонцевыхПробелов",
                "src/Исправленный.bsl",
                "file contained tabs",
            ),
            ScenarioResult::hard_failure(
                "ПроверкаНецензурныхСлов",
                "src/Плохой.bsl",
                "matched dictionary word 'плохоеСлово' at line 1",
            )
            .with_source_span(SourceSpan::new(7, 13)),
            ScenarioResult::modified(
                "УдалениеЛишнихКонцевыхПробелов",
                "src/Исправленный.bsl",
                "removed trailing spaces or tabs",
            ),
        ],
    }
}
