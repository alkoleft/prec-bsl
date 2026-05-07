use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn exec_rules_processes_multiple_source_roots_with_source_root_context() {
    let repo = TempRepo::new("multiple_roots");
    let first = PathBuf::from("configuration/Configuration/Configuration.mdo");
    let second = PathBuf::from("extensions/rat/Configuration/Configuration.mdo");
    write_file(repo.path().join(&first), unsorted_edt_configuration());
    write_file(repo.path().join(&second), unsorted_edt_configuration());

    let output = run_prec_bsl(
        external_cwd(),
        [
            "exec-rules",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "configuration,extensions/rat",
            "--rules",
            "СортировкаСостава,УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert!(output.status.success(), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("prec-bsl exec-rules: processed 2 file(s)"));
    assert!(stdout.contains("Modified files:"));
    assert!(stdout.contains("configuration/Configuration/Configuration.mdo"));
    assert!(stdout.contains("extensions/rat/Configuration/Configuration.mdo"));
    assert!(stdout.contains("СортировкаСостава"));
    assert!(stdout.contains("УдалениеЛишнихКонцевыхПробелов"));
    assert_eq!(
        fs::read_to_string(repo.path().join(first)).unwrap(),
        sorted_edt_configuration()
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(second)).unwrap(),
        sorted_edt_configuration()
    );
}

#[test]
fn exec_rules_reports_missing_source_roots() {
    let repo = TempRepo::new("missing_root");

    let output = run_prec_bsl(
        external_cwd(),
        [
            "exec-rules",
            repo.path().to_str().unwrap(),
            "--source-dir",
            "missing-root",
            "--rules",
            "УдалениеЛишнихКонцевыхПробелов",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("prec-bsl exec-rules: processed 0 file(s)"));
    assert!(stdout.contains("Hard failures:"));
    assert!(stdout.contains("[source-root]"));
    assert!(stdout.contains("missing source root:"));
    assert!(stdout.contains("missing-root"));
}

#[test]
fn exec_rules_uses_explicit_config_and_accumulates_hard_failures_after_traversal() {
    let repo = TempRepo::new("config_and_failures");
    write_file(
        repo.path().join("custom-v8config.json"),
        r#"{
            "Precommt4onecСценарии": {
                "ГлобальныеСценарии": ["ЗапретИспользованияПерейти.os"]
            }
        }"#,
    );
    write_file(
        repo.path().join("src/Первый.bsl"),
        concat!(
            "Процедура Первый()\n",
            "    Перейти ~Метка;\n",
            "~Метка:\n",
            "КонецПроцедуры\n",
        ),
    );
    write_file(
        repo.path().join("src/Второй.bsl"),
        concat!(
            "Процедура Второй()\n",
            "    goto ~Other;\n",
            "~Other:\n",
            "КонецПроцедуры\n",
        ),
    );

    let output = run_prec_bsl(
        external_cwd(),
        [
            "exec-rules",
            repo.path().to_str().unwrap(),
            "--config",
            "custom-v8config.json",
            "--source-dir",
            "src",
        ],
    );

    assert_eq!(output.status.code(), Some(1), "{}", output_text(&output));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("prec-bsl exec-rules: processed 2 file(s)"));
    assert!(stdout.contains("Hard failures:"));
    assert!(stdout.contains("[ЗапретИспользованияПерейти] src/Первый.bsl"));
    assert!(stdout.contains("[ЗапретИспользованияПерейти] src/Второй.bsl"));
    assert_eq!(stdout.matches("goto statement is forbidden").count(), 4);
}

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new(name: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("exec-rules-tests")
            .join(format!("{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

fn run_prec_bsl<const N: usize>(cwd: impl AsRef<Path>, args: [&str; N]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prec-bsl"))
        .current_dir(cwd)
        .args(args)
        .output()
        .unwrap()
}

fn external_cwd() -> PathBuf {
    let path = std::env::current_dir()
        .unwrap()
        .join("target")
        .join("exec-rules-external-cwd");
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_file(path: impl AsRef<Path>, content: &str) {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn unsorted_edt_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.ЯМодуль</commonModules>\n",
        "  <commonModules>CommonModule.АМодуль</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn sorted_edt_configuration() -> &'static str {
    concat!(
        "<mdclass:Configuration>\n",
        "  <languages>Language.Русский</languages>\n",
        "  <commonModules>CommonModule.АМодуль</commonModules>\n",
        "  <commonModules>CommonModule.ЯМодуль</commonModules>\n",
        "</mdclass:Configuration>\n",
    )
}

fn output_text(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}
