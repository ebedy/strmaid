//! Preuves de charge (lot F). Exécution : `cargo test --release --test stress -- --ignored --nocapture`.
//!
//! Chaque test affiche ses mesures sur stdout ; la RSS maximale se mesure séparément
//! avec `/usr/bin/time -v` (le harnais de test partage le processus entre les cas).

use serde_json::{Value, json};
use std::error::Error;
use std::fmt::Write as _;
use std::io::Write;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Instant;
use strmaid::cache::RenderCache;
use strmaid::domain::{
    DiagramBlock, GraphicsProtocol, ResourceLimits, ThemeMode, ViewportGeometry,
};
use strmaid::renderer::{RenderOptions, render_diagram_checked};

type TestResult = Result<(), Box<dyn Error>>;

const BIN: &str = env!("CARGO_BIN_EXE_strmaid");

fn distinct_diagrams_document(count: usize) -> String {
    (0..count).fold(String::new(), |mut doc, i| {
        // L'écriture dans une `String` est infaillible.
        let _ = writeln!(
            doc,
            "Texte {i}\n```mermaid\ngraph TD\n  N{i}A --> N{i}B\n```"
        );
        doc
    })
}

/// Lance le binaire avec `input` sur stdin ; l'écriture se fait dans un thread
/// pour éviter l'interblocage lorsque stdout se remplit avant la fin de l'entrée.
fn run_with_stdin(args: &[&str], input: String) -> Result<Output, Box<dyn Error>> {
    let mut child = Command::new(BIN)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().ok_or("stdin indisponible")?;
    let writer = thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child.wait_with_output()?;
    writer.join().map_err(|_| "thread d'écriture paniqué")??;
    Ok(output)
}

#[test]
#[ignore = "preuve de charge, exécutée en --release"]
fn stress_filter_renders_distinct_diagrams() -> TestResult {
    for count in [10, 100, 1000] {
        let start = Instant::now();
        let output = run_with_stdin(
            &["--no-pager", "-g", "halfblocks", "--format", "json"],
            distinct_diagrams_document(count),
        )?;
        let elapsed = start.elapsed();
        assert!(output.status.success(), "exit {:?}", output.status);

        let document: Value = serde_json::from_slice(&output.stdout)?;
        let summary = &document["summary"];
        assert_eq!(summary["total_diagrams"], json!(count));
        assert_eq!(summary["valid_diagrams"], json!(count));
        println!(
            "filtre {count} diagrammes : {elapsed:?} ({:?}/diagramme)",
            elapsed / u32::try_from(count)?
        );
    }
    Ok(())
}

#[test]
#[ignore = "preuve de charge, exécutée en --release"]
fn stress_cache_stays_within_budget() {
    let limits = ResourceLimits::default();
    let options = RenderOptions::new(
        ThemeMode::Dark,
        GraphicsProtocol::HalfBlocks,
        ViewportGeometry::new(80, 24),
    );
    for i in 0..1000 {
        let block = DiagramBlock::new(format!("graph TD\n  C{i}A --> C{i}B"));
        let (_, valid) = render_diagram_checked(&block, options);
        assert!(valid);
    }

    let cache = RenderCache::global();
    println!(
        "cache après 1000 rendus : {} entrées, {} octets",
        cache.len(),
        cache.size_bytes()
    );
    assert!(cache.len() <= limits.max_cached_diagrams);
    assert!(cache.size_bytes() <= limits.max_cache_bytes);
}

fn large_document_with_oversized_block() -> String {
    let filler = "Ligne de texte Markdown ordinaire pour le test de charge.\n";
    let half = filler.repeat(25 * 1_048_576 / filler.len());
    let oversized = "  A --> B\n".repeat(250_000);
    format!("{half}```mermaid\nflowchart TD\n{oversized}```\n{half}FIN_DU_DOCUMENT\n")
}

#[test]
#[ignore = "preuve de charge, exécutée en --release"]
fn stress_filter_large_document_with_oversized_block() -> TestResult {
    let input = large_document_with_oversized_block();
    let input_len = input.len();
    let start = Instant::now();
    let output = run_with_stdin(&["--no-pager", "-g", "halfblocks"], input)?;
    let elapsed = start.elapsed();

    assert!(output.status.success(), "exit {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Diagramme Mermaid ignoré"));
    assert!(stdout.trim_end().ends_with("FIN_DU_DOCUMENT"));
    println!(
        "document {} Mio avec bloc hors quota : {elapsed:?}",
        input_len / 1_048_576
    );
    Ok(())
}

/// Charge MCP : le comptage des threads lit `/proc/<pid>/status`, propre à Linux.
#[cfg(target_os = "linux")]
mod mcp_load {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, ChildStdin};
    use std::time::Duration;

    /// Client MCP séquentiel : une requête, une ligne de réponse.
    struct McpClient {
        child: Child,
        stdin: ChildStdin,
        stdout: BufReader<std::process::ChildStdout>,
    }

    impl McpClient {
        fn spawn() -> Result<Self, Box<dyn Error>> {
            let mut child = Command::new(BIN)
                .arg("mcp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let stdin = child.stdin.take().ok_or("stdin indisponible")?;
            let stdout = BufReader::new(child.stdout.take().ok_or("stdout indisponible")?);
            Ok(Self {
                child,
                stdin,
                stdout,
            })
        }

        fn call(&mut self, request: &Value) -> Result<(Value, Duration), Box<dyn Error>> {
            let start = Instant::now();
            writeln!(self.stdin, "{request}")?;
            self.stdin.flush()?;
            let mut line = String::new();
            self.stdout.read_line(&mut line)?;
            Ok((serde_json::from_str(&line)?, start.elapsed()))
        }

        /// Nombre de threads du serveur lu dans `/proc` (Linux).
        fn threads(&self) -> Result<usize, Box<dyn Error>> {
            let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id()))?;
            let line = status
                .lines()
                .find(|l| l.starts_with("Threads:"))
                .ok_or("champ Threads absent")?;
            Ok(line.trim_start_matches("Threads:").trim().parse()?)
        }
    }

    impl Drop for McpClient {
        fn drop(&mut self) {
            // Arrêt best-effort du serveur : il peut déjà être terminé.
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    fn render_request(id: usize, source: &str, timeout_ms: u64) -> Value {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": "strmaid_render",
                "arguments": { "source": source, "timeout_ms": timeout_ms }
            }
        })
    }

    fn pathological_source() -> String {
        (0..5000).fold(String::from("flowchart TD\n"), |mut source, i| {
            // L'écriture dans une `String` est infaillible.
            let _ = writeln!(source, "  P{i} --> P{}", (i * 7 + 3) % 5000);
            source
        })
    }

    fn percentile(sorted: &[Duration], pct: usize) -> Duration {
        let idx = (sorted.len() * pct / 100).min(sorted.len().saturating_sub(1));
        sorted.get(idx).copied().unwrap_or_default()
    }

    /// Issue d'une requête légitime : servie, refusée par le plafond d'orphelins, ou autre échec.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Outcome {
        Served,
        RefusedByOrphanCap,
        Failed,
    }

    fn outcome(response: &Value) -> Outcome {
        if response["result"]["isError"] != json!(true) {
            return Outcome::Served;
        }
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default();
        if text.contains("rendus abandonnés") {
            return Outcome::RefusedByOrphanCap;
        }
        Outcome::Failed
    }

    fn legit_request(id: usize) -> Value {
        render_request(id, &format!("graph TD\n  M{id}A --> M{id}B"), 5000)
    }

    /// Relance une requête légitime toutes les 100 ms jusqu'à ce qu'elle soit servie.
    fn wait_for_recovery(client: &mut McpClient) -> Result<Duration, Box<dyn Error>> {
        let start = Instant::now();
        for attempt in 0..100 {
            let (response, _) = client.call(&legit_request(10_000 + attempt))?;
            if outcome(&response) == Outcome::Served {
                return Ok(start.elapsed());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err("service non rétabli après 10 s".into())
    }

    #[test]
    #[ignore = "preuve de charge, exécutée en --release"]
    fn stress_mcp_sequential_requests_bound_threads() -> TestResult {
        let limits = ResourceLimits::default();
        let pathological = pathological_source();
        let mut client = McpClient::spawn()?;
        client.call(&json!({"jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}}))?;

        let mut latencies = Vec::with_capacity(1000);
        let mut legit_outcomes = Vec::with_capacity(900);
        let (mut max_threads, mut pathological_errors) = (0, 0);
        for id in 1..=1000 {
            let is_pathological = id % 10 == 0;
            let request = if is_pathological {
                render_request(id, &pathological, 100)
            } else {
                legit_request(id)
            };
            let (response, latency) = client.call(&request)?;
            latencies.push(latency);
            if is_pathological {
                pathological_errors += usize::from(outcome(&response) != Outcome::Served);
            } else {
                legit_outcomes.push(outcome(&response));
            }
            max_threads = max_threads.max(client.threads()?);
        }
        let recovery = wait_for_recovery(&mut client)?;

        let count = |kind: Outcome| legit_outcomes.iter().filter(|o| **o == kind).count();
        latencies.sort_unstable();
        println!(
            "mcp 1000 requêtes : p50 {:?}, p99 {:?}, max {:?} ; pathologiques en échec {pathological_errors}/100 ; \
             légitimes servies {}, refusées par plafond {}, autres échecs {} ; pic de threads {max_threads} ; \
             service rétabli en {recovery:?}",
            percentile(&latencies, 50),
            percentile(&latencies, 99),
            latencies.last().copied().unwrap_or_default(),
            count(Outcome::Served),
            count(Outcome::RefusedByOrphanCap),
            count(Outcome::Failed),
        );
        assert_eq!(pathological_errors, 100);
        assert_eq!(
            count(Outcome::Failed),
            0,
            "seul le plafond peut refuser une requête légitime"
        );
        // Fil principal + rendus orphelins plafonnés + un rendu légitime encore en cours de sortie.
        assert!(max_threads <= limits.max_orphan_renders + 2);
        Ok(())
    }
}
