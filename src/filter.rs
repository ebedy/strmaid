use crate::domain::{
    CliError, DiagramBlock, JsonDocumentOutput, JsonStreamItem, JsonStreamSummary, OutputFormat,
    ResourceLimits, sanitize_passthrough_text,
};
use crate::renderer::{self, RenderOptions};
use crate::stream::{LossyLines, StreamItem, StreamStateMachine, report_invalid_utf8_once};
use std::borrow::Cow;
use std::io::{BufRead, Write};

/// Exécute le rendu direct d'un unique bloc de diagramme (`--block-only`).
///
/// Strippe les balises Markdown éventuelles (` ```mermaid ... ``` `) et renvoie :
/// - `Ok(true)` si le rendu a abouti avec succès,
/// - `Ok(false)` si le bloc présente une erreur de syntaxe ou de rendu (fallback affiché).
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur I/O, ou `CliError::ResourceLimit` en cas de dépassement de taille.
pub fn run_block_only<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    options: RenderOptions,
) -> Result<bool, CliError> {
    let raw_content = read_bounded_string(reader, options.limits.max_diagram_bytes)?;
    let diagram = DiagramBlock::from_raw(&raw_content);

    match options.format {
        OutputFormat::Human => write_block_human(&mut writer, &diagram, options),
        OutputFormat::Ndjson => write_block_ndjson(&mut writer, &diagram, options),
        OutputFormat::Json => write_block_json(&mut writer, &diagram, options),
    }
}

fn read_bounded_string<R: BufRead>(mut reader: R, max_bytes: usize) -> Result<String, CliError> {
    let mut buffer = String::new();
    let mut total_read = 0_usize;
    let mut line = String::new();

    while reader
        .read_line(&mut line)
        .map_err(|err| CliError::Io(err.to_string()))?
        > 0
    {
        total_read = total_read.saturating_add(line.len());
        if total_read > max_bytes {
            return Err(CliError::ResourceLimit(format!(
                "dépassement de la taille limite du diagramme ({total_read} > {max_bytes} octets)"
            )));
        }
        buffer.push_str(&line);
        line.clear();
    }

    Ok(buffer)
}

fn write_block_human<W: Write>(
    writer: &mut W,
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> Result<bool, CliError> {
    let (rendered, is_valid) = renderer::render_diagram_checked(diagram, options);
    write!(writer, "{rendered}").map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(is_valid)
}

fn write_block_ndjson<W: Write>(
    writer: &mut W,
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> Result<bool, CliError> {
    let item = renderer::analyze_and_render_diagram(diagram, options, 0);
    let is_valid = match &item {
        JsonStreamItem::Diagram { valid, .. } => *valid,
        JsonStreamItem::Text { .. } => true,
    };
    serde_json::to_writer(&mut *writer, &item).map_err(|err| CliError::Io(err.to_string()))?;
    writeln!(writer).map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(is_valid)
}

fn write_block_json<W: Write>(
    writer: &mut W,
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> Result<bool, CliError> {
    let item = renderer::analyze_and_render_diagram(diagram, options, 0);
    let is_valid = match &item {
        JsonStreamItem::Diagram { valid, .. } => *valid,
        JsonStreamItem::Text { .. } => true,
    };
    let doc = build_json_document(vec![item], options);
    serde_json::to_writer_pretty(&mut *writer, &doc)
        .map_err(|err| CliError::Io(err.to_string()))?;
    writeln!(writer).map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(is_valid)
}

/// Exécute le traitement en mode filtre Unix composable (stdin -> stdout).
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur de lecture ou d'écriture,
/// ou `CliError::ResourceLimit` en cas de dépassement de ressource.
pub fn run_filter<R: BufRead, W: Write>(
    reader: R,
    writer: W,
    options: RenderOptions,
) -> Result<(), CliError> {
    match options.format {
        OutputFormat::Human => run_human_filter(reader, writer, options),
        OutputFormat::Ndjson => run_ndjson_filter(reader, writer, options),
        OutputFormat::Json => run_json_filter(reader, writer, options),
    }
}

/// Parcourt le flux d'entrée ligne à ligne (UTF-8 tolérant) et transmet chaque élément
/// produit par la machine à états, y compris celui émis à l'EOF.
fn drive_stream<R: BufRead>(
    reader: R,
    limits: ResourceLimits,
    mut on_item: impl FnMut(&StreamItem) -> Result<(), CliError>,
) -> Result<(), CliError> {
    let mut machine = StreamStateMachine::with_limits(limits);
    let mut utf8_reported = false;
    for line_result in LossyLines::new(reader) {
        let line = line_result?;
        utf8_reported = report_invalid_utf8_once(&line, utf8_reported);
        if let Some(item) = machine.process_line(&line.text)? {
            on_item(&item)?;
        }
    }
    machine.finish().map_or(Ok(()), |item| on_item(&item))
}

fn run_human_filter<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    options: RenderOptions,
) -> Result<(), CliError> {
    drive_stream(reader, options.limits, |item| {
        write_human_item(&mut writer, item, options)
    })?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(())
}

fn write_human_item<W: Write>(
    writer: &mut W,
    item: &StreamItem,
    options: RenderOptions,
) -> Result<(), CliError> {
    match item {
        StreamItem::Text(text) => {
            let relayed = passthrough_text(text, options);
            writeln!(writer, "{relayed}").map_err(|err| CliError::Io(err.to_string()))?;
        }
        StreamItem::Diagram(diagram) => {
            let rendered = renderer::render_diagram(diagram, options);
            write!(writer, "{rendered}").map_err(|err| CliError::Io(err.to_string()))?;
            writer
                .flush()
                .map_err(|err| CliError::Io(err.to_string()))?;
        }
    }
    Ok(())
}

/// Texte Markdown relayé : séquences actives neutralisées sauf `--raw-passthrough`.
fn passthrough_text(text: &str, options: RenderOptions) -> Cow<'_, str> {
    if options.raw_passthrough {
        return Cow::Borrowed(text);
    }
    sanitize_passthrough_text(text)
}

fn run_ndjson_filter<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    options: RenderOptions,
) -> Result<(), CliError> {
    let mut diagram_index = 0_usize;
    drive_stream(reader, options.limits, |item| {
        write_ndjson_item(&mut writer, item, options, &mut diagram_index)
    })?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(())
}

fn write_ndjson_item<W: Write>(
    writer: &mut W,
    item: &StreamItem,
    options: RenderOptions,
    diagram_index: &mut usize,
) -> Result<(), CliError> {
    let json_item = convert_to_json_item(item, options, diagram_index);
    serde_json::to_writer(&mut *writer, &json_item).map_err(|err| CliError::Io(err.to_string()))?;
    writeln!(writer).map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(())
}

fn run_json_filter<R: BufRead, W: Write>(
    reader: R,
    mut writer: W,
    options: RenderOptions,
) -> Result<(), CliError> {
    let mut diagram_index = 0_usize;
    let mut items = Vec::new();
    drive_stream(reader, options.limits, |item| {
        items.push(convert_to_json_item(item, options, &mut diagram_index));
        Ok(())
    })?;

    let doc = build_json_document(items, options);
    serde_json::to_writer_pretty(&mut writer, &doc).map_err(|err| CliError::Io(err.to_string()))?;
    writeln!(writer).map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(())
}

fn convert_to_json_item(
    item: &StreamItem,
    options: RenderOptions,
    diagram_index: &mut usize,
) -> JsonStreamItem {
    match item {
        StreamItem::Text(text) => JsonStreamItem::Text {
            content: text.clone(),
        },
        StreamItem::Diagram(diagram) => {
            let index = *diagram_index;
            *diagram_index = diagram_index.saturating_add(1);
            renderer::analyze_and_render_diagram(diagram, options, index)
        }
    }
}

fn build_json_document(items: Vec<JsonStreamItem>, options: RenderOptions) -> JsonDocumentOutput {
    let summary = compute_summary(&items);
    JsonDocumentOutput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        format: OutputFormat::Json,
        theme: options.theme,
        protocol: options.protocol,
        items,
        summary,
    }
}

fn compute_summary(items: &[JsonStreamItem]) -> JsonStreamSummary {
    let mut total_diagrams = 0_usize;
    let mut valid_diagrams = 0_usize;
    let mut invalid_diagrams = 0_usize;

    for item in items {
        if let JsonStreamItem::Diagram { valid, .. } = item {
            total_diagrams = total_diagrams.saturating_add(1);
            if *valid {
                valid_diagrams = valid_diagrams.saturating_add(1);
            } else {
                invalid_diagrams = invalid_diagrams.saturating_add(1);
            }
        }
    }

    JsonStreamSummary {
        total_items: items.len(),
        total_diagrams,
        valid_diagrams,
        invalid_diagrams,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GraphicsProtocol, ResourceLimits, ThemeMode, ViewportGeometry};
    use std::io::Cursor;

    #[test]
    fn test_run_filter_with_markdown_and_diagram() {
        let input = "# Test Header\nSome regular markdown\n```mermaid\ngraph LR\n  A --> B\n```\nTrailing text\n";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );

        let res = run_filter(reader, &mut output, options);
        assert!(res.is_ok());
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("# Test Header"));
        assert!(output_str.contains("Some regular markdown"));
        assert!(output_str.contains("Trailing text"));
    }

    #[test]
    fn test_run_filter_neutralizes_active_sequences_in_text_but_keeps_sgr() {
        let input = "\x1b[32mvert\x1b[0m \x1b]52;c;ZXZpbA==\x07\x1b]0;PWNED\x07fin\n";
        let mut output = Vec::new();
        let res = run_filter(Cursor::new(input), &mut output, RenderOptions::default());
        assert!(res.is_ok());
        assert_eq!(
            String::from_utf8_lossy(&output),
            "\x1b[32mvert\x1b[0m fin\n"
        );
    }

    #[test]
    fn test_run_filter_tolerates_invalid_utf8_bytes() {
        let input: &[u8] = b"ok\n\xff\xfe\nsuite\n";
        let mut output = Vec::new();
        let res = run_filter(Cursor::new(input), &mut output, RenderOptions::default());
        assert!(res.is_ok());
        assert_eq!(
            String::from_utf8_lossy(&output),
            "ok\n\u{fffd}\u{fffd}\nsuite\n"
        );
    }

    #[test]
    fn test_run_filter_raw_passthrough_relays_text_verbatim() {
        let input = "a\x1b]0;titre\x07b\n";
        let mut output = Vec::new();
        let options = RenderOptions::default().with_raw_passthrough(true);
        let res = run_filter(Cursor::new(input), &mut output, options);
        assert!(res.is_ok());
        assert_eq!(String::from_utf8_lossy(&output), input);
    }

    #[test]
    fn test_run_filter_with_oversized_diagram_returns_error() {
        let input = "```mermaid\n123456789\n```\n";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let limits = ResourceLimits {
            max_diagram_bytes: 4,
            ..ResourceLimits::default()
        };
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let res = run_filter(reader, &mut output, options);
        assert!(matches!(res, Err(CliError::ResourceLimit(_))));
    }

    #[test]
    fn test_run_filter_ndjson_with_valid_and_invalid_diagrams() {
        let input = "Intro\n```mermaid\ngraph LR\n  A --> B\n```\nMiddle\n```mermaid\ngraph TD\n  syntax error $$$\n```\nOutro\n";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        )
        .with_format(OutputFormat::Ndjson);

        let res = run_filter(reader, &mut output, options);
        assert!(res.is_ok());

        let output_str = String::from_utf8_lossy(&output);
        let lines: Vec<&str> = output_str.lines().collect();
        assert_eq!(lines.len(), 5);

        let item0: Result<JsonStreamItem, _> = serde_json::from_str(lines[0]);
        assert!(item0.is_ok());
        if let Ok(item) = item0 {
            assert_eq!(
                item,
                JsonStreamItem::Text {
                    content: "Intro".to_string()
                }
            );
        }

        let item1: Result<JsonStreamItem, _> = serde_json::from_str(lines[1]);
        assert!(item1.is_ok());
        if let Ok(JsonStreamItem::Diagram {
            index,
            valid,
            dimensions,
            error,
            ..
        }) = item1
        {
            assert_eq!(index, 0);
            assert!(valid);
            assert!(dimensions.is_some());
            assert!(error.is_none());
        }

        let item2: Result<JsonStreamItem, _> = serde_json::from_str(lines[2]);
        assert!(item2.is_ok());
        if let Ok(item) = item2 {
            assert_eq!(
                item,
                JsonStreamItem::Text {
                    content: "Middle".to_string()
                }
            );
        }

        let item3: Result<JsonStreamItem, _> = serde_json::from_str(lines[3]);
        assert!(item3.is_ok());
        if let Ok(JsonStreamItem::Diagram {
            index,
            valid,
            dimensions,
            error,
            ..
        }) = item3
        {
            assert_eq!(index, 1);
            assert!(!valid);
            assert!(dimensions.is_none());
            assert!(error.is_some());
            if let Some(err) = error {
                assert_eq!(err.line, Some(2));
            }
        }

        let item4: Result<JsonStreamItem, _> = serde_json::from_str(lines[4]);
        assert!(item4.is_ok());
        if let Ok(item) = item4 {
            assert_eq!(
                item,
                JsonStreamItem::Text {
                    content: "Outro".to_string()
                }
            );
        }
    }

    #[test]
    fn test_run_filter_json_full_document() {
        let input =
            "Intro\n```mermaid\ngraph LR\n  A --> B\n```\n```mermaid\ninvalid diagram\n```\n";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        )
        .with_format(OutputFormat::Json);

        let res = run_filter(reader, &mut output, options);
        assert!(res.is_ok());

        let doc_result: Result<JsonDocumentOutput, _> = serde_json::from_slice(&output);
        assert!(doc_result.is_ok());
        if let Ok(doc) = doc_result {
            assert_eq!(doc.format, OutputFormat::Json);
            assert_eq!(doc.summary.total_items, 3);
            assert_eq!(doc.summary.total_diagrams, 2);
            assert_eq!(doc.summary.valid_diagrams, 1);
            assert_eq!(doc.summary.invalid_diagrams, 1);
        }
    }

    #[test]
    fn test_run_block_only_valid_raw_mermaid() {
        let input = "graph LR\n  A --> B\n";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );

        let res = run_block_only(reader, &mut output, options);
        assert_eq!(res, Ok(true));
        let output_str = String::from_utf8_lossy(&output);
        assert!(!output_str.is_empty());
        assert!(!output_str.contains("Rendu Mermaid indisponible"));
    }

    #[test]
    fn test_run_block_only_markdown_fences_stripped() {
        let input = "```mermaid\ngraph LR\n  A --> B\n```";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );

        let res = run_block_only(reader, &mut output, options);
        assert_eq!(res, Ok(true));
        let output_str = String::from_utf8_lossy(&output);
        assert!(!output_str.is_empty());
    }

    #[test]
    fn test_run_block_only_invalid_syntax_returns_false() {
        let input = "graph TD\n  syntax error $$$";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );

        let res = run_block_only(reader, &mut output, options);
        assert_eq!(res, Ok(false));
        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("Rendu Mermaid indisponible"));
        assert!(output_str.contains("syntax error $$$"));
    }

    #[test]
    fn test_run_block_only_exceeds_limits_returns_error() {
        let input = "graph LR\n  A --> B";
        let reader = Cursor::new(input);
        let mut output = Vec::new();
        let limits = ResourceLimits {
            max_diagram_bytes: 5,
            ..ResourceLimits::default()
        };
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let res = run_block_only(reader, &mut output, options);
        assert!(matches!(res, Err(CliError::ResourceLimit(_))));
    }

    #[test]
    fn test_run_block_only_ndjson_and_json() {
        let input = "```mermaid\ngraph LR\n  A --> B\n```";
        let options_ndjson = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        )
        .with_format(OutputFormat::Ndjson);

        let mut out_ndjson = Vec::new();
        let res_ndjson = run_block_only(Cursor::new(input), &mut out_ndjson, options_ndjson);
        assert_eq!(res_ndjson, Ok(true));

        let item: Result<JsonStreamItem, _> = serde_json::from_slice(&out_ndjson);
        assert!(item.is_ok());

        let options_json = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        )
        .with_format(OutputFormat::Json);

        let mut out_json = Vec::new();
        let res_json = run_block_only(Cursor::new(input), &mut out_json, options_json);
        assert_eq!(res_json, Ok(true));

        let doc: Result<JsonDocumentOutput, _> = serde_json::from_slice(&out_json);
        assert!(doc.is_ok());
    }
}
