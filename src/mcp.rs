use crate::domain::{CliError, DiagramBlock, ResourceLimits, ThemeMode, strip_mermaid_fences};
use crate::mermaid;
use crate::rasterizer;
use crate::renderer;
use crate::stream::{LossyLines, StreamItem, StreamStateMachine, report_invalid_utf8_once};
use base64::prelude::*;
use resvg::tiny_skia::PixmapRef;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

/// Requête standard JSON-RPC 2.0 reçue sur stdin.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// Réponse standard JSON-RPC 2.0 émise sur stdout.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Erreur standard JSON-RPC 2.0.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Dimensions d'une image matricielle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

/// Résultat produit par l'outil `strmaid_render`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderOutput {
    pub svg: String,
    pub png_base64: String,
    pub dimensions: Dimensions,
}

/// Détail d'erreur de validation Mermaid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateErrorDetail {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

/// Résultat produit par l'outil `strmaid_validate`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateOutput {
    pub valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ValidateErrorDetail>,
}

/// Bloc détecté par l'outil `strmaid_detect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedBlock {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
}

/// Résultat produit par l'outil `strmaid_detect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectOutput {
    pub diagram_count: usize,
    pub blocks: Vec<DetectedBlock>,
}

/// Contenu textuel d'un résultat d'outil MCP.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Résultat d'un appel d'outil MCP (`tools/call`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolCallContent>,
    #[serde(rename = "isError")]
    pub is_error: bool,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub structured: Option<Value>,
}

/// Exécute la boucle de traitement du serveur MCP sur les flux I/O fournis.
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur de lecture ou d'écriture irrécupérable.
pub fn run_mcp_server<R: BufRead, W: Write>(reader: R, mut writer: W) -> Result<(), CliError> {
    let mut utf8_reported = false;
    for line_result in LossyLines::new(reader) {
        let line = line_result?;
        utf8_reported = report_invalid_utf8_once(&line, utf8_reported);
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            continue;
        }
        process_mcp_line(trimmed, &mut writer)?;
    }
    Ok(())
}

fn process_mcp_line<W: Write>(line: &str, writer: &mut W) -> Result<(), CliError> {
    let req: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(parsed) => parsed,
        Err(err) => {
            let error_resp =
                make_error_response(Value::Null, -32700, format!("Parse error: {err}"));
            return write_json_response(writer, &error_resp);
        }
    };

    if let Some(resp) = handle_rpc_request(req) {
        write_json_response(writer, &resp)?;
    }
    Ok(())
}

fn write_json_response<W: Write>(
    writer: &mut W,
    response: &JsonRpcResponse,
) -> Result<(), CliError> {
    serde_json::to_writer(&mut *writer, response).map_err(|err| CliError::Io(err.to_string()))?;
    writeln!(writer).map_err(|err| CliError::Io(err.to_string()))?;
    writer
        .flush()
        .map_err(|err| CliError::Io(err.to_string()))?;
    Ok(())
}

fn handle_rpc_request(req: JsonRpcRequest) -> Option<JsonRpcResponse> {
    let id = req.id.unwrap_or(Value::Null);

    // Les notifications sans id (ex: notifications/initialized) ne renvoient pas de réponse
    if id.is_null() && req.method.starts_with("notifications/") {
        return None;
    }

    Some(dispatch_method(id, &req.method, &req.params))
}

fn dispatch_method(id: Value, method: &str, params: &Value) -> JsonRpcResponse {
    match method {
        "initialize" => handle_initialize(id),
        "ping" => make_success_response(id, json!({})),
        "tools/list" => handle_tools_list(id),
        "tools/call" => handle_tools_call(id, params),
        _ => make_error_response(id, -32601, format!("Method not found: {method}")),
    }
}

fn handle_initialize(id: Value) -> JsonRpcResponse {
    let version = env!("CARGO_PKG_VERSION");
    let result = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "strmaid",
            "version": version
        }
    });
    make_success_response(id, result)
}

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = json!({
        "tools": [
            {
                "name": "strmaid_render",
                "description": "Génère un SVG et un PNG matriciel Base64 depuis une source Mermaid avec dimensions",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Code source du diagramme Mermaid" },
                        "theme": { "type": "string", "description": "Thème visuel (dark, light, neutral, amber, phosphor, neon, mono - alias retro-* supportés)", "default": "dark" },
                        "width": { "type": "integer", "description": "Largeur cible en colonnes (clampée entre 20 et 1000)", "default": 80 },
                        "engine": { "type": "string", "description": "Moteur vectoriel Mermaid (mermaid-svg par défaut, merman si activé)", "default": "mermaid-svg" },
                        "timeout_ms": { "type": "integer", "description": "Délai maximal d'exécution en millisecondes (défaut 5000, 0 pour désactiver)", "default": 5000 }
                    },
                    "required": ["source"]
                }
            },
            {
                "name": "strmaid_validate",
                "description": "Valide la syntaxe d'un diagramme Mermaid et renvoie les détails d'erreur éventuels",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "source": { "type": "string", "description": "Code source Mermaid à valider" }
                    },
                    "required": ["source"]
                }
            },
            {
                "name": "strmaid_detect",
                "description": "Scanne un texte Markdown et extrait tous les blocs de diagramme Mermaid avec métadonnées",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "markdown": { "type": "string", "description": "Document Markdown à analyser" }
                    },
                    "required": ["markdown"]
                }
            }
        ]
    });
    make_success_response(id, tools)
}

fn handle_tools_call(id: Value, params: &Value) -> JsonRpcResponse {
    let tool_name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

    let result = match tool_name {
        "strmaid_render" => call_strmaid_render(&arguments),
        "strmaid_validate" => call_strmaid_validate(&arguments),
        "strmaid_detect" => call_strmaid_detect(&arguments),
        _ => {
            return make_error_response(id, -32602, format!("Unknown tool: {tool_name}"));
        }
    };

    match serde_json::to_value(&result) {
        Ok(val) => make_success_response(id, val),
        Err(err) => make_error_response(id, -32603, format!("Serialization error: {err}")),
    }
}

#[derive(Debug, Clone, Copy)]
struct McpRenderParams<'a> {
    source: &'a str,
    theme_name: Option<&'a str>,
    width_col: Option<u64>,
    engine_name: Option<&'a str>,
    timeout_ms: Option<u64>,
}

fn call_strmaid_render(args: &Value) -> ToolCallResult {
    let Some(source) = args.get("source").and_then(Value::as_str) else {
        return make_tool_error("Paramètre requis manquant: 'source'");
    };
    let params = McpRenderParams {
        source,
        theme_name: args.get("theme").and_then(Value::as_str),
        width_col: args.get("width").and_then(Value::as_u64),
        engine_name: args.get("engine").and_then(Value::as_str),
        timeout_ms: args.get("timeout_ms").and_then(Value::as_u64),
    };

    match render_diagram_to_mcp_payload(&params) {
        Ok(output) => {
            let json_str = serde_json::to_string(&output).unwrap_or_default();
            let structured = serde_json::to_value(&output).ok();
            ToolCallResult {
                content: vec![ToolCallContent {
                    content_type: "text".to_string(),
                    text: json_str,
                }],
                is_error: false,
                structured,
            }
        }
        Err(err) => make_tool_error(&format!("Échec de génération Mermaid: {err}")),
    }
}

fn render_diagram_to_mcp_payload(params: &McpRenderParams<'_>) -> Result<RenderOutput, String> {
    let cleaned = strip_mermaid_fences(params.source);
    let theme = params
        .theme_name
        .map_or(ThemeMode::Dark, ThemeMode::from_str_name);
    let engine_str = params.engine_name.unwrap_or("mermaid-svg");
    let engine = mermaid::engine_by_name(engine_str).map_err(|e| e.to_string())?;

    let target_cols = u16::try_from(params.width_col.unwrap_or(80).clamp(20, 1000)).unwrap_or(80);
    let target_width_px = u32::from(target_cols).saturating_mul(8);

    let block = DiagramBlock::from_raw(&cleaned);
    let timeout_dur = match params.timeout_ms {
        Some(0) => None,
        Some(ms) => Some(std::time::Duration::from_millis(ms)),
        None => Some(std::time::Duration::from_millis(5000)),
    };

    let svg = renderer::run_with_render_timeout(
        move || {
            engine
                .render_svg(&block, theme)
                .map_err(|err| CliError::MermaidSyntax(err.message))
        },
        &ResourceLimits::default().with_render_timeout(timeout_dur),
    )
    .map_err(|e| e.to_string())?;

    let rasterized = rasterizer::rasterize_svg(
        &svg,
        target_width_px,
        ResourceLimits::default().max_raster_pixels,
    )
    .map_err(|e| e.to_string())?;

    let pixmap_ref = PixmapRef::from_bytes(&rasterized.rgba, rasterized.width, rasterized.height)
        .ok_or_else(|| "Buffer RGBA invalide".to_string())?;
    let png_bytes = pixmap_ref
        .encode_png()
        .map_err(|e| format!("Échec encodage PNG: {e}"))?;
    let png_base64 = BASE64_STANDARD.encode(&png_bytes);

    Ok(RenderOutput {
        svg,
        png_base64,
        dimensions: Dimensions {
            width: rasterized.width,
            height: rasterized.height,
        },
    })
}

fn call_strmaid_validate(args: &Value) -> ToolCallResult {
    let Some(source) = args.get("source").and_then(Value::as_str) else {
        return make_tool_error("Paramètre requis manquant: 'source'");
    };

    let cleaned = strip_mermaid_fences(source);
    let block = DiagramBlock::from_raw(&cleaned);
    let output = match mermaid::render_to_svg_detailed(&block, ThemeMode::Dark) {
        Ok(_) => ValidateOutput {
            valid: true,
            error: None,
        },
        Err(e) => ValidateOutput {
            valid: false,
            error: Some(ValidateErrorDetail {
                message: e.message,
                line: e.line,
                kind: e.kind,
            }),
        },
    };

    let json_str = serde_json::to_string(&output).unwrap_or_default();
    let structured = serde_json::to_value(&output).ok();
    ToolCallResult {
        content: vec![ToolCallContent {
            content_type: "text".to_string(),
            text: json_str,
        }],
        is_error: false,
        structured,
    }
}

fn call_strmaid_detect(args: &Value) -> ToolCallResult {
    let Some(markdown) = args.get("markdown").and_then(Value::as_str) else {
        return make_tool_error("Paramètre requis manquant: 'markdown'");
    };

    let output = detect_markdown_diagrams(markdown);
    let json_str = serde_json::to_string(&output).unwrap_or_default();
    let structured = serde_json::to_value(&output).ok();
    ToolCallResult {
        content: vec![ToolCallContent {
            content_type: "text".to_string(),
            text: json_str,
        }],
        is_error: false,
        structured,
    }
}

fn detect_markdown_diagrams(markdown: &str) -> DetectOutput {
    let mut machine = StreamStateMachine::with_limits(ResourceLimits::default());
    let mut blocks = Vec::new();

    for line in markdown.lines() {
        if let Some(StreamItem::Diagram(block)) = machine.process_line(line) {
            blocks.push(build_detected_block(&block));
        }
    }
    if let Some(StreamItem::Diagram(block)) = machine.finish() {
        blocks.push(build_detected_block(&block));
    }

    DetectOutput {
        diagram_count: blocks.len(),
        blocks,
    }
}

fn build_detected_block(block: &DiagramBlock) -> DetectedBlock {
    DetectedBlock {
        content: block.as_str().to_string(),
        title: block.title().map(ToString::to_string),
        theme: block.theme_override().map(|t| t.to_string()),
        width: block.width_override(),
    }
}

fn make_tool_error(message: &str) -> ToolCallResult {
    ToolCallResult {
        content: vec![ToolCallContent {
            content_type: "text".to_string(),
            text: message.to_string(),
        }],
        is_error: true,
        structured: None,
    }
}

fn make_success_response(id: Value, result: Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: Some(result),
        error: None,
    }
}

fn make_error_response(id: Value, code: i32, message: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0".to_string(),
        id,
        result: None,
        error: Some(JsonRpcError { code, message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_mcp_initialize() {
        let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(input), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        assert_eq!(resp.id, json!(1));
        assert!(resp.error.is_none());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["serverInfo"]["name"], "strmaid");
    }

    #[test]
    fn test_mcp_server_survives_invalid_utf8_line() {
        let input: &[u8] =
            b"\xff\xfe garbage\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"initialize\",\"params\":{}}\n";
        let mut output = Vec::new();
        assert!(run_mcp_server(Cursor::new(input), &mut output).is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let responses: Vec<JsonRpcResponse> = out_str
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        assert_eq!(responses.len(), 2);
        assert!(responses[0].error.is_some());
        assert_eq!(responses[1].id, json!(7));
        assert!(responses[1].error.is_none());
    }

    #[test]
    fn test_mcp_tools_list() {
        let input = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(input), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        assert_eq!(resp.id, json!(2));
        let tools = resp.result.unwrap_or(Value::Null)["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0]["name"], "strmaid_render");
        assert_eq!(tools[1]["name"], "strmaid_validate");
        assert_eq!(tools[2]["name"], "strmaid_detect");
    }

    #[test]
    fn test_mcp_validate_tool_valid_and_invalid() {
        let valid_input = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"strmaid_validate","arguments":{"source":"flowchart TD\n  A --> B"}}}"#;
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(valid_input), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["isError"], false);
        assert_eq!(result["valid"], true);

        let invalid_input = r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"strmaid_validate","arguments":{"source":"flowchart TD\n  invalid syntax @@@"}}}"#;
        let mut output2 = Vec::new();
        let run_status = run_mcp_server(Cursor::new(invalid_input), &mut output2);
        assert!(run_status.is_ok());

        let out_str2 = String::from_utf8(output2).unwrap_or_default();
        let resp2: JsonRpcResponse =
            serde_json::from_str(&out_str2).unwrap_or_else(|_| unreachable!());
        let result2 = resp2.result.unwrap_or(Value::Null);
        assert_eq!(result2["valid"], false);
        assert!(result2["error"]["message"].is_string());
    }

    #[test]
    fn test_mcp_detect_tool() {
        let md =
            "# Rapport\n\n```mermaid title=\"Architecture\"\nflowchart LR\n  X --> Y\n```\n\nFin.";
        let input = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "strmaid_detect",
                "arguments": {
                    "markdown": md
                }
            }
        });
        let line = serde_json::to_string(&input).unwrap_or_default();
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(line), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["diagram_count"], 1);
        assert_eq!(result["blocks"][0]["title"], "Architecture");
    }

    #[test]
    fn test_mcp_render_tool() {
        let input = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "strmaid_render",
                "arguments": {
                    "source": "flowchart TD\n  A --> B",
                    "theme": "dark",
                    "width": 60
                }
            }
        });
        let line = serde_json::to_string(&input).unwrap_or_default();
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(line), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["isError"], false);
        assert!(result["svg"].as_str().unwrap_or("").contains("<svg"));
        assert_ne!(result["png_base64"].as_str().unwrap_or(""), "");
        assert!(result["dimensions"]["width"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn test_mcp_render_tool_with_engine_and_retro_theme() {
        let input = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "strmaid_render",
                "arguments": {
                    "source": "flowchart TD\n  A --> B",
                    "theme": "retro-amber",
                    "engine": "mermaid-svg",
                    "timeout_ms": 3000,
                    "width": 80
                }
            }
        });
        let line = serde_json::to_string(&input).unwrap_or_default();
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(line), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["isError"], false);
        assert!(result["svg"].as_str().unwrap_or("").contains("<svg"));
    }

    #[test]
    fn test_mcp_render_tool_invalid_engine_reports_error() {
        let input = json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "strmaid_render",
                "arguments": {
                    "source": "flowchart TD\n  A --> B",
                    "engine": "unknown-engine"
                }
            }
        });
        let line = serde_json::to_string(&input).unwrap_or_default();
        let mut output = Vec::new();
        let res = run_mcp_server(Cursor::new(line), &mut output);
        assert!(res.is_ok());

        let out_str = String::from_utf8(output).unwrap_or_default();
        let resp: JsonRpcResponse =
            serde_json::from_str(&out_str).unwrap_or_else(|_| unreachable!());
        let result = resp.result.unwrap_or(Value::Null);
        assert_eq!(result["isError"], true);
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .contains("Moteur Mermaid inconnu")
        );
    }
}
