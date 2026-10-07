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
fn test_cli_rejects_out_of_range_width() -> TestResult {
    for width in ["0", "8000"] {
        let mut cmd = strmaid_cmd()?;
        cmd.arg("--width").arg(width);
        cmd.assert()
            .code(2)
            .stderr(predicate::str::contains("10..=1000"));
    }
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
#[cfg(unix)]
fn test_cli_run_echo_command() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("run").arg("--").arg("echo").arg("hello from pty");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("hello from pty"));
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_cli_run_intercepts_diagram() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("run")
        .arg("--")
        .arg("sh")
        .arg("-c")
        .arg("echo 'intro'; echo '```mermaid'; echo 'flowchart TD'; echo '  A --> B'; echo '```'; echo 'outro'");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("intro"))
        .stdout(predicate::str::contains("outro"))
        .stdout(predicate::str::contains("```mermaid").not());
    Ok(())
}

#[test]
#[cfg(unix)]
fn test_cli_run_propagates_child_exit_code() -> TestResult {
    for (script, expected_code) in [("exit 42", 42), ("exit 0", 0), ("exit 1", 1)] {
        let mut cmd = strmaid_cmd()?;
        cmd.arg("run").arg("--").arg("sh").arg("-c").arg(script);
        cmd.assert().code(expected_code);
    }
    Ok(())
}

#[test]
fn test_cli_fallback_output_contains_no_osc_sequence() -> TestResult {
    let input = "intro\x1b]52;c;ZXZpbA==\x07\n```mermaid\nxyz \x1b]0;PWNED\x07 $$$\n```\nfin\n";
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--no-pager").arg("-g").arg("halfblocks");
    let output = cmd.write_stdin(input).output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());
    assert!(stdout.contains("Rendu Mermaid indisponible"));
    assert!(stdout.contains("fin"));
    assert!(!stdout.contains("\x1b]"), "OSC sur stdout : {stdout:?}");
    assert!(!stderr.contains("\x1b]"), "OSC sur stderr : {stderr:?}");
    Ok(())
}

#[test]
fn test_cli_raw_passthrough_relays_markdown_text() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--no-pager").arg("--raw-passthrough");
    cmd.write_stdin("a\x1b]0;titre\x07b\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("a\x1b]0;titre\x07b"));
    Ok(())
}

#[test]
fn test_cli_filter_tolerates_invalid_utf8_with_single_warning() -> TestResult {
    let mut cmd = strmaid_cmd()?;
    cmd.arg("--no-pager");
    let output = cmd
        .write_stdin(b"ok\n\xff\xfe\nsuite\n\xc3\n".to_vec())
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr : {stderr}");
    assert!(stdout.contains("suite"));
    assert_eq!(stderr.matches("UTF-8").count(), 1, "stderr : {stderr}");
    Ok(())
}
