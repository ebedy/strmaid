use assert_cmd::Command;
use predicates::prelude::*;
use std::error::Error;

type TestResult = Result<(), Box<dyn Error>>;

fn strmaid_cmd() -> Result<Command, Box<dyn Error>> {
    let cmd = Command::cargo_bin("strmaid")?;
    Ok(cmd)
}

#[test]
fn test_cli_rejects_invalid_theme() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--theme").arg("invalid_theme");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("invalid value 'invalid_theme'"));
    Ok(())
}

#[test]
fn test_cli_rejects_zero_width() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--width").arg("0");
    cmd.assert().failure().stderr(predicate::str::contains(
        "la largeur --width doit être strictement supérieure à 0",
    ));
    Ok(())
}

#[test]
fn test_cli_rejects_conflicting_pager_and_json() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--pager").arg("--format").arg("json");
    cmd.assert().failure().stderr(predicate::str::contains(
        "incompatible avec les formats machine-readable",
    ));
    Ok(())
}

#[test]
fn test_cli_block_only_valid() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--block-only");
    cmd.write_stdin("graph TD\n  A --> B\n");
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
    Ok(())
}

#[test]
fn test_cli_block_only_invalid_syntax_exits_failure() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--block-only");
    cmd.write_stdin("graph TD\n  $$$syntax error$$$\n");
    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("Rendu Mermaid indisponible"));
    Ok(())
}

#[test]
fn test_cli_doctor_human() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("doctor");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Strmaid Doctor"))
        .stdout(predicate::str::contains("OPÉRATIONNEL"));
    Ok(())
}

#[test]
fn test_cli_doctor_json() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("doctor").arg("--format").arg("json");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"healthy\": true"))
        .stdout(predicate::str::contains("\"svg_generated\": true"));
    Ok(())
}

#[test]
fn test_cli_pipe_stream_filter() -> TestResult {
    let markdown = "# Titre\n\n```mermaid\ngraph LR\n  A --> B\n```\n\nFin du flux.\n";
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--no-pager");
    cmd.write_stdin(markdown);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("# Titre"))
        .stdout(predicate::str::contains("Fin du flux."));
    Ok(())
}

#[test]
fn test_cli_mcp_initialize() -> TestResult {
    let req = r#"{"jsonrpc":"2.0","id":100,"method":"initialize","params":{}}"#;
    let mut cmd = strmaid_cmd()?;
    cmd.arg("mcp");
    cmd.write_stdin(format!("{req}\n"));
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"strmaid\""))
        .stdout(predicate::str::contains(
            "\"protocolVersion\":\"2024-11-05\"",
        ));
    Ok(())
}

#[test]
fn test_cli_run_echo_command() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    if cfg!(windows) {
        cmd.arg("run")
            .arg("--")
            .arg("cmd")
            .arg("/c")
            .arg("echo hello from pty");
    } else {
        cmd.arg("run").arg("--").arg("echo").arg("hello from pty");
    }
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello from pty"));
    Ok(())
}

#[test]
fn test_cli_run_intercepts_diagram() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    if cfg!(windows) {
        cmd.arg("run")
            .arg("--")
            .arg("cmd")
            .arg("/c")
            .arg("echo intro & echo ```mermaid & echo flowchart TD & echo   A --^> B & echo ``` & echo outro");
    } else {
        cmd.arg("run")
            .arg("--")
            .arg("sh")
            .arg("-c")
            .arg("echo 'intro'; echo '```mermaid'; echo 'flowchart TD'; echo '  A --> B'; echo '```'; echo 'outro'");
    }
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("intro"))
        .stdout(predicate::str::contains("outro"))
        .stdout(predicate::str::contains("```mermaid").not());
    Ok(())
}
