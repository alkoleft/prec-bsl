//! Parity-приёмка `ПроверкаПереносовСтрок` на контрольном корпусе yaxunit
//! (issue #5): прогон детектора `find_line_break_violations` на всех `.bsl`
//! под `exts`, `tests`, `fixtures` корпуса и сравнение с закоммиченным
//! baseline `tests/fixtures/parity/ilb-fingerprint.txt`; плюс corpus-smoke
//! фикстера `fix_line_breaks` (0 паник + идемпотентность на каждом файле).
//!
//! Parity — это снапшот к запиненному коммиту корпуса `PINNED_CORPUS_COMMIT`
//! (d21ce6ff), а не к текущему состоянию рабочей копии: файлы извлекаются из
//! коммита через `git archive` во временный каталог (git archive только читает
//! репозиторий корпуса, никаких записей в него), поэтому локальные
//! модификации рабочей копии корпуса не влияют на результат. Если локальный
//! корпус не содержит запиненный коммит (upstream обновился и выпал из
//! локальной истории), тест skip-ается с сообщением; регенерация baseline —
//! по шапке `tests/fixtures/parity/ilb-fingerprint.txt`. Corpus-smoke
//! фикстера работает с тем же извлечённым снапшотом коммита.
//!
//! Путь корпуса задаётся env `YAXUNIT_CORPUS` (default
//! `/data/home/korav/projects/yaxunit`). Если git-репозиторий корпуса
//! отсутствует, тест молча пропускается с eprintln-сообщением (паттерн
//! `rat_acceptance.rs`) — на CI без корпуса тест не падает, на машине с
//! корпусом гоняется.
//!
//! Регенерация baseline описана в шапке baseline-файла.

use std::fs;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_YAXUNIT_CORPUS: &str = "/data/home/korav/projects/yaxunit";
const BASELINE_PATH: &str = "tests/fixtures/parity/ilb-fingerprint.txt";
/// Запиненный коммит корпуса, к которому снят baseline. Parity-прогон
/// извлекает содержимое этого коммита через `git archive`, поэтому локальные
/// правки рабочей копии корпуса не влияют на результат.
const PINNED_CORPUS_COMMIT: &str = "d21ce6ff1b553e82fb1a4e96b752c01b45ffa10c";

fn resolved_corpus_path() -> PathBuf {
    std::env::var_os("YAXUNIT_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_YAXUNIT_CORPUS))
}

fn yaxunit_corpus() -> Option<PathBuf> {
    let path = resolved_corpus_path();
    if !path.join(".git").exists() {
        if std::env::var_os("YAXUNIT_CORPUS").is_some() {
            panic!(
                "YAXUNIT_CORPUS is set explicitly, but {} is not a git repository: an \
                 explicitly requested corpus must not silently skip",
                path.display()
            );
        }
        return None;
    }
    Some(path)
}

/// RAII-гвард временного каталога снапшота: каталог удаляется при выходе из
/// области видимости — в конце теста и при панике assert'а.
struct SnapshotDir(PathBuf);

impl Drop for SnapshotDir {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!(
                "warning: failed to remove snapshot dir {}: {error}",
                self.0.display()
            );
        }
    }
}

impl Deref for SnapshotDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

/// Извлечение снапшота запиненного коммита корпуса (`git archive` + `tar`) во
/// временный каталог `prec-bsl-ilb-parity-<pid>-<tag>`; тег различает parity-
/// и smoke-тесты, которые cargo запускает потоками одного процесса (pid
/// совпадает). Возвращает гвард с корнем извлечённых `exts`/`tests`/
/// `fixtures` (каталог чистится при drop, включая паники). `None` — skip:
/// коммит отсутствует в локальном репозитории (corpus drift), `git`/`tar`
/// недоступны или упали; в skip-путях каталог тоже подчищается.
fn extract_pinned_snapshot(corpus: &Path, tag: &str) -> Option<SnapshotDir> {
    let snapshot_dir =
        std::env::temp_dir().join(format!("prec-bsl-ilb-parity-{}-{tag}", std::process::id()));
    if let Err(error) = fs::create_dir_all(&snapshot_dir) {
        eprintln!(
            "skipping yaxunit line-break corpus tests: failed to create snapshot dir {}: {error}",
            snapshot_dir.display()
        );
        return None;
    }
    // Гвард подчищает каталог при любом выходе: skip-return ниже, паника в
    // тесте или конец теста после возврата Some(..) вызывающему.
    let snapshot = SnapshotDir(snapshot_dir);

    let tar_path = snapshot.join("corpus.tar");
    let archive_output = match Command::new("git")
        .arg("-C")
        .arg(corpus)
        .arg("archive")
        .arg("--format=tar")
        .arg("-o")
        .arg(&tar_path)
        .arg(PINNED_CORPUS_COMMIT)
        .arg("exts")
        .arg("tests")
        .arg("fixtures")
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "skipping yaxunit line-break corpus tests: failed to run git in {}: {error}",
                corpus.display()
            );
            return None;
        }
    };
    if !archive_output.status.success() {
        eprintln!(
            "skipping yaxunit line-break corpus tests: corpus {} does not contain pinned \
             commit {PINNED_CORPUS_COMMIT} (upstream moved on); refresh baseline per header \
             of {BASELINE_PATH}",
            corpus.display()
        );
        return None;
    }

    let extract_output = match Command::new("tar")
        .arg("-xf")
        .arg(&tar_path)
        .arg("-C")
        .arg(&*snapshot)
        .output()
    {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "skipping yaxunit line-break corpus tests: tar command is unavailable: {error}"
            );
            return None;
        }
    };
    // tar-файл больше не нужен независимо от результата распаковки.
    let _ = fs::remove_file(&tar_path);
    if !extract_output.status.success() {
        eprintln!(
            "skipping yaxunit line-break corpus tests: tar -xf failed:\n{}",
            String::from_utf8_lossy(&extract_output.stderr)
        );
        return None;
    }

    if !snapshot.join("exts").is_dir() {
        eprintln!(
            "skipping yaxunit line-break corpus tests: pinned commit {PINNED_CORPUS_COMMIT} of \
             corpus {} does not contain exts/tests/fixtures roots",
            corpus.display()
        );
        return None;
    }

    Some(snapshot)
}

/// Рекурсивный сбор `.bsl`-файлов (read-only обход) в детерминированном
/// порядке: сначала отсортированный список файлов, находки внутри файла —
/// по порядку строк. Ошибки обхода (права, битые симлинки) — ошибка теста,
/// не тихий пропуск частичного списка.
fn collect_bsl_files(root: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_bsl_files(&path, files)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("bsl")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(())
}

/// Сбор `.bsl`-файлов снапшота корпуса (общий для parity- и smoke-тестов).
/// Отсутствующие корневые `exts`/`tests`/`fixtures` — допустимы (корень
/// пропускается, как раньше).
fn corpus_bsl_files(corpus: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for root in ["exts", "tests", "fixtures"] {
        let root_path = corpus.join(root);
        if root_path.is_dir() {
            collect_bsl_files(&root_path, &mut files)?;
        }
    }
    files.sort();
    Ok(files)
}

/// Fingerprint-строки текущего прогона: `path|rule|message` на каждую находку,
/// пути — относительные от корня снапшота (= корня корпуса), разделители
/// нормализованы в `/`.
fn collect_current_fingerprint(corpus: &Path) -> Vec<String> {
    let files = corpus_bsl_files(corpus)
        .unwrap_or_else(|error| panic!("failed to walk corpus {}: {error}", corpus.display()));

    let settings = prec_bsl::line_break::LineBreakSettings::default_settings();
    let mut fingerprint = Vec::new();
    for file in &files {
        let input = fs::read_to_string(file).unwrap_or_else(|error| {
            panic!("failed to read corpus file {}: {error}", file.display())
        });
        let violations = prec_bsl::line_break::find_line_break_violations(&input, &settings);
        let relative = file
            .strip_prefix(corpus)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        for violation in &violations {
            fingerprint.push(format!(
                "{relative}|ПроверкаПереносовСтрок|incorrect line break at line {}",
                violation.line
            ));
        }
    }
    fingerprint
}

/// Строки baseline без комментариев (`#`-префикс) и пустых строк.
fn baseline_fingerprint_lines() -> Vec<String> {
    let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE_PATH);
    let baseline = fs::read_to_string(&baseline_path).unwrap_or_else(|error| {
        panic!(
            "failed to read parity baseline {}: {error}",
            baseline_path.display()
        )
    });

    baseline
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

#[test]
fn yaxunit_line_break_parity_matches_committed_baseline() {
    let Some(corpus) = yaxunit_corpus() else {
        eprintln!(
            "skipping yaxunit line-break parity: corpus {} is not available; \
             set YAXUNIT_CORPUS to run",
            resolved_corpus_path().display()
        );
        return;
    };

    let Some(snapshot_dir) = extract_pinned_snapshot(&corpus, "parity") else {
        return;
    };

    let baseline = baseline_fingerprint_lines();
    assert!(
        !baseline.is_empty(),
        "parity baseline must contain at least one fingerprint line"
    );

    let current = collect_current_fingerprint(&snapshot_dir);

    // Полная эквивалентность последовательностей (не только счётчик): любые
    // новые/пропавшие находки и сдвиги номеров строк — падение теста.
    assert_eq!(
        current.len(),
        baseline.len(),
        "line-break parity finding count changed: baseline {} findings vs current {}",
        baseline.len(),
        current.len()
    );

    let mut first_difference = None;
    for (index, (current_line, baseline_line)) in current.iter().zip(baseline.iter()).enumerate() {
        if current_line != baseline_line {
            first_difference = Some((index, current_line.clone(), baseline_line.clone()));
            break;
        }
    }

    if let Some((index, current_line, baseline_line)) = first_difference {
        // Дополнительно показаем множественную разницу для читаемого отчёта.
        let missing: Vec<_> = baseline
            .iter()
            .filter(|line| !current.contains(line))
            .cloned()
            .collect();
        let added: Vec<_> = current
            .iter()
            .filter(|line| !baseline.contains(line))
            .cloned()
            .collect();
        panic!(
            "line-break parity fingerprint diverged at entry {index}:\n  \
             baseline: {baseline_line}\n  current:  {current_line}\n\
             findings only in baseline ({}): {missing:?}\n\
             findings only in current ({}): {added:?}",
            missing.len(),
            added.len()
        );
    }
}

/// Corpus-smoke фикстера `fix_line_breaks` (ревью M3): на каждом `.bsl` файла
/// корпуса (1) фикс не паникует и (2) идемпотентен — `fix(fix(x)) == fix(x)`.
///
/// Этот тест ловит регрессии вида C4 (паника мультибайтного среза после
/// правки строки) и H2 (цепочки носителей, сходящиеся только за два прогона),
/// которые не покрывает детекторный parity-baseline. Прогон read-only:
/// результаты фикса живут только в памяти, снапшот корпуса не изменяется.
#[test]
fn yaxunit_line_break_fix_corpus_smoke_idempotent() {
    let Some(corpus) = yaxunit_corpus() else {
        eprintln!(
            "skipping yaxunit line-break fix smoke: corpus {} is not available; \
             set YAXUNIT_CORPUS to run",
            resolved_corpus_path().display()
        );
        return;
    };

    let Some(snapshot_dir) = extract_pinned_snapshot(&corpus, "smoke") else {
        return;
    };

    let settings = prec_bsl::line_break::LineBreakSettings::default_settings();
    let files = corpus_bsl_files(&snapshot_dir).unwrap_or_else(|error| {
        panic!(
            "failed to walk corpus snapshot {}: {error}",
            snapshot_dir.display()
        )
    });
    assert!(
        !files.is_empty(),
        "yaxunit corpus must contain at least one .bsl file for the fix smoke test"
    );

    for file in &files {
        let input = fs::read_to_string(file).unwrap_or_else(|error| {
            panic!("failed to read corpus file {}: {error}", file.display())
        });

        // Паника внутри fix_line_breaks (напр. срез не по границе символа)
        // напрямую валит тест с путём файла в контексте ниже.
        let once = prec_bsl::line_break::fix_line_breaks(&input, &settings);
        let twice = prec_bsl::line_break::fix_line_breaks(&once, &settings);
        assert_eq!(
            twice,
            once,
            "fix_line_breaks must be idempotent on corpus file {}",
            file.display()
        );
    }
}

// CR3 (PR #17): явно заданный YAXUNIT_CORPUS без валидного git-репозитория —
// это ошибка окружения, а не тихий skip (ложный зелёный при явном запросе
// недопустим).
#[test]
fn explicit_corpus_without_git_repo_must_fail_not_skip() {
    let explicit_dir = std::env::temp_dir()
        .join("prec-bsl-ilb-corpus-cr3")
        .join(format!("plain-dir-{}", std::process::id()));
    fs::create_dir_all(&explicit_dir).expect("CR3 test dir must be created");
    // Внутри — `exts`, но НЕ `.git`: раньше такой корпус тихо skip-ался.
    fs::create_dir_all(explicit_dir.join("exts")).expect("CR3 exts dir must be created");

    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("yaxunit_line_break_parity_matches_committed_baseline")
        .env("YAXUNIT_CORPUS", &explicit_dir)
        .output()
        .expect("nested test process must start");
    fs::remove_dir_all(&explicit_dir).ok();

    assert!(
        !output.status.success(),
        "an explicitly set YAXUNIT_CORPUS without .git must fail, not silently skip:\n\
         stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    // libtest захватывает panic-сообщение вложенного теста в stdout
    // (секция `---- <test> stdout ----`), поэтому ищем в обоих потоках.
    let output_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output_text.contains("is not a git repository"),
        "the failure must name the invalid corpus path:\n{output_text}"
    );
}

// CR4 (PR #17): ошибки обхода корпуса (права, битые симлинки) — ошибка
// теста, не тихий частичный список файлов.
//
// Root-safety (PR #17 round 2): chmod 000 не мешает root (CI-контейнеры,
// CAP_DAC_OVERRIDE) читать каталог — fs::read_dir ok → expect_err паниковал
// бы и валил `cargo test --workspace` в окружении без корпуса. Поэтому
// после снятия прав тест зондирует фактическую читаемость каталога: если
// процесс читает chmod-000 каталоги, Unix-часть skip-ается с сообщением
// (контракт CR4 непроверяем в этом окружении), остальные кейсы остаются.
#[test]
fn corpus_walk_errors_must_fail_not_skip() {
    // Корневой каталог без прав на чтение: fs::read_dir возвращает
    // PermissionDenied — раньше глотался, сейчас валит тест.
    #[cfg(unix)]
    {
        let unwalkable = std::env::temp_dir()
            .join("prec-bsl-ilb-corpus-cr4")
            .join(format!("no-read-{}", std::process::id()));
        fs::create_dir_all(&unwalkable).expect("CR4 test dir must be created");
        let snapshot_root = unwalkable.join("snapshot");
        fs::create_dir_all(snapshot_root.join("exts")).expect("CR4 exts dir must be created");
        set_no_read_permissions(&snapshot_root.join("exts"));

        // Зонд root-окружения: процесс с CAP_DAC_OVERRIDE читает chmod-000
        // каталоги — PermissionDenied не наступит, контракт CR4 в этом
        // окружении непроверяем.
        if fs::read_dir(snapshot_root.join("exts")).is_ok() {
            reset_permissions(&snapshot_root.join("exts"));
            fs::remove_dir_all(&unwalkable).ok();
            eprintln!(
                "skipping permission check: process can read chmod-000 dirs \
                 (running as root or with CAP_DAC_OVERRIDE)"
            );
        } else {
            let files = corpus_bsl_files(&snapshot_root);
            reset_permissions(&snapshot_root.join("exts"));
            fs::remove_dir_all(&unwalkable).ok();

            let error = files.expect_err("an unreadable corpus root must fail the walk");
            assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        }
    }

    // Отсутствующие корневые `exts`/`tests`/`fixtures` — допустимы (корень
    // пропускается), как и раньше.
    let empty_root = std::env::temp_dir()
        .join("prec-bsl-ilb-corpus-cr4")
        .join(format!("empty-{}", std::process::id()));
    fs::create_dir_all(&empty_root).expect("CR4 empty dir must be created");
    let files =
        corpus_bsl_files(&empty_root).expect("missing corpus roots must be an acceptable skip");
    assert!(files.is_empty());
    fs::remove_dir_all(&empty_root).ok();
}

// Root-safety (PR #17 round 2): зонд-путь «каталог читается → skip Unix-часть,
// не паниковать» покрыт отдельным кейсом с намеренно читаемым каталогом.
// Project-контракт зонда: читаемый каталог обязан обходиться без ошибок и
// отдавать свои файлы (тот же walk `corpus_bsl_files`, которым parity-тест
// собирает корпус).
#[cfg(unix)]
#[test]
fn corpus_walk_probe_treats_readable_dir_as_readable() {
    let readable = std::env::temp_dir()
        .join("prec-bsl-ilb-corpus-cr4")
        .join(format!("readable-{}", std::process::id()));
    fs::create_dir_all(readable.join("exts")).expect("probe exts dir must be created");
    fs::write(readable.join("exts").join("module.bsl"), "А = 1;\n")
        .expect("probe module must be written");

    let files =
        corpus_bsl_files(&readable).expect("a readable corpus root must walk without errors");
    assert_eq!(
        files.len(),
        1,
        "the probe dir must yield its single .bsl file"
    );
    fs::remove_dir_all(&readable).ok();
}

/// Права `000` на каталог: read_dir падает PermissionDenied.
#[cfg(unix)]
fn set_no_read_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o000))
        .expect("chmod 000 must succeed on the test dir");
}

#[cfg(unix)]
fn reset_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .expect("chmod 755 must succeed on the test dir");
}
