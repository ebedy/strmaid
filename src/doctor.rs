use crate::domain::{
    CliError, DiagramBlock, GraphicsProtocol, OutputFormat, ThemeMode, ViewportGeometry,
};
use crate::mermaid;
use crate::protocol;
use crate::rasterizer;
use crossterm::terminal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::io::{self, Write};

const DIAGNOSTIC_ENV_VARS: [&str; 8] = [
    "KITTY_WINDOW_ID",
    "TERM_PROGRAM",
    "COLORTERM",
    "TERM",
    "WT_SESSION",
    "NO_COLOR",
    "TMUX",
    "STY",
];

const SAMPLE_MERMAID: &str = "flowchart LR\n    A-->B";

/// Diagnostic des variables d'environnement du terminal hôte.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentDiagnostic {
    pub variables: BTreeMap<String, Option<String>>,
}

/// Diagnostic des dimensions et de la géométrie du terminal hôte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalDiagnostic {
    pub columns: u16,
    pub rows: u16,
    pub target_columns: u16,
}

/// Diagnostic de la chaîne vectorielle de rendu Mermaid et rasterisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineDiagnostic {
    pub sample_diagram: String,
    pub svg_generated: bool,
    pub rasterized: bool,
    pub raster_dimensions: Option<(u32, u32)>,
    pub error: Option<String>,
    /// Nombre de faces de polices système disponibles pour les libellés.
    pub font_faces: usize,
}

/// Rapport complet d'inspection émis par `strmaid doctor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub detected_protocol: GraphicsProtocol,
    pub terminal: TerminalDiagnostic,
    pub environment: EnvironmentDiagnostic,
    pub pipeline: PipelineDiagnostic,
    pub healthy: bool,
}

impl DoctorReport {
    /// Collecte l'ensemble des diagnostics système et teste la chaîne de rendu.
    #[must_use]
    pub fn collect() -> Self {
        let detected_protocol = protocol::detect_protocol(None);
        let terminal = collect_terminal();
        let environment = collect_environment();
        let pipeline = run_pipeline_check();
        let healthy = pipeline.svg_generated && pipeline.rasterized;

        Self {
            detected_protocol,
            terminal,
            environment,
            pipeline,
            healthy,
        }
    }
}

fn collect_environment() -> EnvironmentDiagnostic {
    let mut variables = BTreeMap::new();
    for &var in &DIAGNOSTIC_ENV_VARS {
        variables.insert(var.to_string(), env::var(var).ok());
    }
    EnvironmentDiagnostic { variables }
}

fn collect_terminal() -> TerminalDiagnostic {
    let (columns, rows) = terminal::size().unwrap_or((80, 24));
    let target_columns = ViewportGeometry::new(columns, rows).target_columns();
    TerminalDiagnostic {
        columns,
        rows,
        target_columns,
    }
}

fn run_pipeline_check() -> PipelineDiagnostic {
    let block = DiagramBlock::new(SAMPLE_MERMAID.to_string());
    let svg = match mermaid::render_to_svg(&block, ThemeMode::Dark) {
        Ok(s) => s,
        Err(e) => return pipeline_failure(e.to_string(), false),
    };

    match rasterizer::rasterize_svg(&svg, 400, 8_000_000) {
        Ok(img) => pipeline_success((img.width, img.height)),
        Err(e) => pipeline_failure(e.to_string(), true),
    }
}

fn pipeline_failure(err: String, svg_ok: bool) -> PipelineDiagnostic {
    PipelineDiagnostic {
        sample_diagram: SAMPLE_MERMAID.to_string(),
        svg_generated: svg_ok,
        rasterized: false,
        raster_dimensions: None,
        error: Some(err),
        font_faces: rasterizer::loaded_font_count(),
    }
}

fn pipeline_success(dims: (u32, u32)) -> PipelineDiagnostic {
    PipelineDiagnostic {
        sample_diagram: SAMPLE_MERMAID.to_string(),
        svg_generated: true,
        rasterized: true,
        raster_dimensions: Some(dims),
        error: None,
        font_faces: rasterizer::loaded_font_count(),
    }
}

/// Émet le diagnostic complet du terminal hôte vers `stdout`.
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur d'écriture.
pub fn run_doctor(format: OutputFormat) -> Result<bool, CliError> {
    let stdout = io::stdout();
    run_doctor_to_writer(format, stdout.lock())
}

/// Émet le diagnostic vers un writer configurable (pour tests et découplage).
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur d'écriture.
pub fn run_doctor_to_writer<W: Write>(
    format: OutputFormat,
    mut writer: W,
) -> Result<bool, CliError> {
    let report = DoctorReport::collect();
    if format == OutputFormat::Json || format == OutputFormat::Ndjson {
        write_json_report(&report, &mut writer)?;
    } else {
        write_human_report(&report, &mut writer).map_err(|e| CliError::Io(e.to_string()))?;
    }
    Ok(report.healthy)
}

fn write_json_report<W: Write>(report: &DoctorReport, writer: &mut W) -> Result<(), CliError> {
    let json_bytes = serde_json::to_vec_pretty(report)
        .map_err(|e| CliError::Io(format!("Erreur sérialisation diagnostic: {e}")))?;
    writer
        .write_all(&json_bytes)
        .map_err(|e| CliError::Io(e.to_string()))?;
    writer
        .write_all(b"\n")
        .map_err(|e| CliError::Io(e.to_string()))?;
    Ok(())
}

fn write_human_report<W: Write>(report: &DoctorReport, writer: &mut W) -> io::Result<()> {
    writeln!(
        writer,
        "\x1b[1m🩺 Strmaid Doctor — Diagnostic Système\x1b[0m"
    )?;
    writeln!(writer, "======================================")?;
    writeln!(
        writer,
        "Protocole graphique détecté : {:?}",
        report.detected_protocol
    )?;
    writeln!(
        writer,
        "Résolution terminal         : {} colonnes x {} lignes (cible: {} cols)",
        report.terminal.columns, report.terminal.rows, report.terminal.target_columns
    )?;
    writeln!(writer, "\nVariables d'environnement :")?;
    for (k, v) in &report.environment.variables {
        let val_display = v.as_deref().unwrap_or("<non définie>");
        writeln!(writer, "  • {k:<16} : {val_display}")?;
    }
    write_human_pipeline_section(report, writer)?;
    Ok(())
}

fn write_human_pipeline_section<W: Write>(report: &DoctorReport, writer: &mut W) -> io::Result<()> {
    writeln!(writer, "\nChaîne de rendu vectorielle :")?;
    let svg_status = if report.pipeline.svg_generated {
        "OK"
    } else {
        "ÉCHEC"
    };
    writeln!(writer, "  • Génération SVG (mermaid-svg) : {svg_status}")?;

    let rast_status = if report.pipeline.rasterized {
        let (w, h) = report.pipeline.raster_dimensions.unwrap_or((0, 0));
        format!("OK ({w}x{h} px)")
    } else {
        "ÉCHEC".to_string()
    };
    writeln!(writer, "  • Rasterisation (resvg)       : {rast_status}")?;

    let fonts_status = match report.pipeline.font_faces {
        0 => "AUCUNE (repli AsciiBox pour les diagrammes)".to_string(),
        count => format!("{count} faces chargées"),
    };
    writeln!(writer, "  • Polices système             : {fonts_status}")?;

    let health_badge = if report.healthy {
        "\x1b[32m✅ OPÉRATIONNEL\x1b[0m"
    } else {
        "\x1b[31m❌ DÉGRADÉ\x1b[0m"
    };
    writeln!(writer, "  • Statut d'intégrité           : {health_badge}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_report_collect_is_healthy() {
        let report = DoctorReport::collect();
        assert!(report.healthy);
        assert!(report.pipeline.svg_generated);
        assert!(report.pipeline.rasterized);
        assert!(report.pipeline.raster_dimensions.is_some());
    }

    #[test]
    fn test_doctor_run_to_writer_human() {
        let mut buffer = Vec::new();
        let res = run_doctor_to_writer(OutputFormat::Human, &mut buffer);
        assert!(res.is_ok());
        let output = String::from_utf8_lossy(&buffer);
        assert!(output.contains("Strmaid Doctor"));
        assert!(output.contains("Protocole graphique détecté"));
        assert!(output.contains("OPÉRATIONNEL"));
    }

    #[test]
    fn test_doctor_run_to_writer_json() {
        let mut buffer = Vec::new();
        let res = run_doctor_to_writer(OutputFormat::Json, &mut buffer);
        assert!(res.is_ok());
        let parsed: Result<serde_json::Value, _> = serde_json::from_slice(&buffer);
        assert!(parsed.is_ok());
        let json_val = parsed.unwrap_or_default();
        assert_eq!(json_val["healthy"], true);
        assert_eq!(json_val["pipeline"]["svg_generated"], true);
        assert_eq!(
            json_val["pipeline"]["font_faces"],
            rasterizer::loaded_font_count()
        );
    }

    #[test]
    fn test_doctor_reports_multiplexer_variables() {
        let report = DoctorReport::collect();
        assert!(report.environment.variables.contains_key("TMUX"));
        assert!(report.environment.variables.contains_key("STY"));
    }

    #[test]
    fn test_doctor_human_output_reports_font_faces() {
        let mut buffer = Vec::new();
        assert!(run_doctor_to_writer(OutputFormat::Human, &mut buffer).is_ok());
        let output = String::from_utf8_lossy(&buffer);
        assert!(output.contains("Polices système"));
    }
}
