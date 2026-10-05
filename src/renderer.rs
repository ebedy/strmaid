use crate::cache::{RenderCache, RenderCacheKey};
use crate::domain::{
    CliError, DiagramBlock, DiagramDimensions, DiagramEngineType, DiagramErrorDetail,
    GraphicsProtocol, JsonStreamItem, OutputFormat, RasterizedImage, ResourceLimits, ThemeMode,
    ViewportGeometry,
};
use crate::mermaid;
use crate::protocol::{asciibox, halfblock, iterm2, kitty};
use crate::rasterizer;

use std::borrow::Cow;

/// Options de configuration pour le rendu d'un bloc de diagramme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderOptions {
    pub theme: ThemeMode,
    pub protocol: GraphicsProtocol,
    pub viewport: ViewportGeometry,
    pub limits: ResourceLimits,
    pub format: OutputFormat,
    pub auto_orient: bool,
    pub engine: DiagramEngineType,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self::new(
            ThemeMode::Dark,
            GraphicsProtocol::Kitty,
            ViewportGeometry::new(80, 24),
        )
    }
}

impl RenderOptions {
    #[must_use]
    pub fn new(theme: ThemeMode, protocol: GraphicsProtocol, viewport: ViewportGeometry) -> Self {
        Self {
            theme,
            protocol,
            viewport,
            limits: ResourceLimits::default(),
            format: OutputFormat::Human,
            auto_orient: true,
            engine: DiagramEngineType::default(),
        }
    }

    #[must_use]
    pub const fn with_limits(
        theme: ThemeMode,
        protocol: GraphicsProtocol,
        viewport: ViewportGeometry,
        limits: ResourceLimits,
    ) -> Self {
        Self {
            theme,
            protocol,
            viewport,
            limits,
            format: OutputFormat::Human,
            auto_orient: true,
            engine: DiagramEngineType::MermaidSvg,
        }
    }

    #[must_use]
    pub const fn with_all(
        theme: ThemeMode,
        protocol: GraphicsProtocol,
        viewport: ViewportGeometry,
        limits: ResourceLimits,
        format: OutputFormat,
    ) -> Self {
        Self {
            theme,
            protocol,
            viewport,
            limits,
            format,
            auto_orient: true,
            engine: DiagramEngineType::MermaidSvg,
        }
    }

    #[must_use]
    pub const fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    #[must_use]
    pub const fn with_auto_orient(mut self, auto_orient: bool) -> Self {
        self.auto_orient = auto_orient;
        self
    }

    #[must_use]
    pub const fn with_engine(mut self, engine: DiagramEngineType) -> Self {
        self.engine = engine;
        self
    }
}

fn resolve_effective_options(diagram: &DiagramBlock, options: RenderOptions) -> RenderOptions {
    let theme = diagram.theme_override().unwrap_or(options.theme);
    let target_cols = diagram
        .width_override()
        .unwrap_or_else(|| options.viewport.target_columns())
        .max(10);
    let viewport = ViewportGeometry::new(target_cols, options.viewport.rows);

    RenderOptions {
        theme,
        protocol: options.protocol,
        viewport,
        limits: options.limits,
        format: options.format,
        auto_orient: options.auto_orient,
        engine: options.engine,
    }
}

fn prepare_diagram_for_rendering(
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> Cow<'_, DiagramBlock> {
    if !options.auto_orient {
        return Cow::Borrowed(diagram);
    }

    let target_cols = options.viewport.columns.max(10);
    let adapted = mermaid::adapt_direction_for_viewport(diagram.as_str(), target_cols);
    match adapted {
        Cow::Owned(new_content) => {
            let block = DiagramBlock::with_metadata(new_content, diagram.metadata().clone());
            Cow::Owned(block)
        }
        Cow::Borrowed(_) => Cow::Borrowed(diagram),
    }
}

/// Analyse et rend un bloc Mermaid sous forme structurée pour agents IA (JSON / NDJSON).
#[must_use]
pub fn analyze_and_render_diagram(
    diagram: &DiagramBlock,
    options: RenderOptions,
    index: usize,
) -> JsonStreamItem {
    let raw_source = diagram.as_str().to_string();
    let effective_options = resolve_effective_options(diagram, options);
    let effective_diagram = prepare_diagram_for_rendering(diagram, effective_options);
    let render_target = effective_diagram.into_owned();
    let timeout = effective_options.limits.render_timeout;

    let res = run_with_render_timeout(
        move || {
            let item = if effective_options.protocol == GraphicsProtocol::AsciiBox {
                render_asciibox_to_json_item(&render_target, effective_options, index)
            } else {
                match mermaid::render_to_svg_detailed_with_engine(
                    &render_target,
                    effective_options.theme,
                    effective_options.engine,
                ) {
                    Ok(svg) => {
                        render_svg_to_json_item(&svg, &render_target, effective_options, index)
                    }
                    Err(err_detail) => JsonStreamItem::Diagram {
                        index,
                        valid: false,
                        title: render_target.title().map(ToString::to_string),
                        dimensions: None,
                        protocol: None,
                        payload: None,
                        error: Some(err_detail),
                        raw_content: render_target.as_str().to_string(),
                    },
                }
            };
            Ok(item)
        },
        timeout,
    );

    let mut item = match res {
        Ok(item) => item,
        Err(err) => JsonStreamItem::Diagram {
            index,
            valid: false,
            title: diagram.title().map(ToString::to_string),
            dimensions: None,
            protocol: None,
            payload: None,
            error: Some(DiagramErrorDetail::new(
                format!("{err}"),
                None,
                Some("RenderTimeout".to_string()),
            )),
            raw_content: raw_source.clone(),
        },
    };

    if let JsonStreamItem::Diagram { raw_content, .. } = &mut item {
        *raw_content = raw_source;
    }
    item
}

fn render_asciibox_to_json_item(
    diagram: &DiagramBlock,
    options: RenderOptions,
    index: usize,
) -> JsonStreamItem {
    let target_cols = options.viewport.columns.max(10);
    let ascii_opts = asciibox::AsciiBoxOptions::new(target_cols, options.theme != ThemeMode::Mono);

    match asciibox::render_asciibox(diagram.as_str(), ascii_opts) {
        Ok(text) => JsonStreamItem::Diagram {
            index,
            valid: true,
            title: diagram.title().map(ToString::to_string),
            dimensions: None,
            protocol: Some(GraphicsProtocol::AsciiBox),
            payload: Some(text),
            error: None,
            raw_content: diagram.as_str().to_string(),
        },
        Err(err_detail) => JsonStreamItem::Diagram {
            index,
            valid: false,
            title: diagram.title().map(ToString::to_string),
            dimensions: None,
            protocol: Some(GraphicsProtocol::AsciiBox),
            payload: None,
            error: Some(err_detail),
            raw_content: diagram.as_str().to_string(),
        },
    }
}

fn render_svg_to_json_item(
    svg: &str,
    diagram: &DiagramBlock,
    options: RenderOptions,
    index: usize,
) -> JsonStreamItem {
    if options.protocol == GraphicsProtocol::Raw {
        return JsonStreamItem::Diagram {
            index,
            valid: true,
            title: diagram.title().map(ToString::to_string),
            dimensions: None,
            protocol: Some(GraphicsProtocol::Raw),
            payload: Some(svg.to_string()),
            error: None,
            raw_content: diagram.as_str().to_string(),
        };
    }

    let target_cols = options.viewport.columns.max(10);
    let target_width_px = u32::from(target_cols) * 8;
    match rasterizer::rasterize_svg(svg, target_width_px, options.limits.max_raster_pixels) {
        Ok(image) => {
            let dimensions = DiagramDimensions::new(image.width, image.height);
            let image = composite_on_theme_background(image, options.theme);
            let payload = encode_image_for_protocol(&image, options);
            JsonStreamItem::Diagram {
                index,
                valid: true,
                title: diagram.title().map(ToString::to_string),
                dimensions: Some(dimensions),
                protocol: Some(options.protocol),
                payload: Some(payload),
                error: None,
                raw_content: diagram.as_str().to_string(),
            }
        }
        Err(err) => JsonStreamItem::Diagram {
            index,
            valid: false,
            title: diagram.title().map(ToString::to_string),
            dimensions: None,
            protocol: None,
            payload: None,
            error: Some(DiagramErrorDetail::new(
                err.to_string(),
                None,
                Some("RasterizeError".to_string()),
            )),
            raw_content: diagram.as_str().to_string(),
        },
    }
}

fn run_with_render_timeout<F, T>(f: F, timeout: Option<std::time::Duration>) -> Result<T, CliError>
where
    F: FnOnce() -> Result<T, CliError> + Send + 'static,
    T: Send + 'static,
{
    let Some(timeout_dur) = timeout else {
        return f();
    };

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let res = f();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(timeout_dur) {
        Ok(res) => res,
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(CliError::ResourceLimit(format!(
            "délai de rendu Mermaid dépassé ({} ms)",
            timeout_dur.as_millis()
        ))),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(CliError::ResourceLimit(
            "interruption anormale du thread de rendu Mermaid".to_string(),
        )),
    }
}

/// Rendu d'un bloc Mermaid avec rapport de validité (succès vs fallback d'erreur).
#[must_use]
pub fn render_diagram_checked(diagram: &DiagramBlock, options: RenderOptions) -> (String, bool) {
    let effective_options = resolve_effective_options(diagram, options);
    let effective_diagram = prepare_diagram_for_rendering(diagram, effective_options);
    let diagram = effective_diagram.as_ref();

    let cache_key = RenderCacheKey::new(
        diagram.as_str(),
        effective_options.theme,
        effective_options.viewport.columns,
        effective_options.protocol,
        effective_options.engine,
    );

    if let Some((cached_payload, is_valid)) = RenderCache::global().get(&cache_key) {
        let title_prefix = diagram.title().map_or_else(String::new, |t| {
            format_title_header(t, effective_options.viewport.columns)
        });
        return (format!("{title_prefix}{cached_payload}"), is_valid);
    }

    let diag_for_render = diagram.clone();
    let timeout = effective_options.limits.render_timeout;

    let render_result = run_with_render_timeout(
        move || {
            if effective_options.protocol == GraphicsProtocol::AsciiBox {
                Ok(render_asciibox_to_terminal(
                    &diag_for_render,
                    effective_options,
                ))
            } else {
                let svg = mermaid::render_to_svg_with_engine(
                    &diag_for_render,
                    effective_options.theme,
                    effective_options.engine,
                )?;
                Ok(render_svg_to_terminal_with_raw(
                    &svg,
                    &diag_for_render,
                    effective_options,
                ))
            }
        },
        timeout,
    );

    match render_result {
        Ok((output, is_valid, raw_payload)) => {
            if is_valid {
                RenderCache::global().insert(cache_key, (raw_payload, true));
            }
            (output, is_valid)
        }
        Err(err) => (format_fallback(diagram, &err), false),
    }
}

fn render_asciibox_to_terminal(
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> (String, bool, String) {
    let target_cols = options.viewport.columns.max(10);
    let ascii_opts = asciibox::AsciiBoxOptions::new(target_cols, options.theme != ThemeMode::Mono);

    match asciibox::render_asciibox(diagram.as_str(), ascii_opts) {
        Ok(text) => {
            let title_prefix = diagram
                .title()
                .map_or_else(String::new, |t| format_title_header(t, target_cols));
            (format!("{title_prefix}{text}"), true, text)
        }
        Err(err_detail) => {
            let cli_err = CliError::MermaidSyntax(err_detail.message);
            (format_fallback(diagram, &cli_err), false, String::new())
        }
    }
}

/// Rendu complet d'un bloc Mermaid vers une chaîne terminale ou un fallback dégradé.
#[must_use]
pub fn render_diagram(diagram: &DiagramBlock, options: RenderOptions) -> String {
    render_diagram_checked(diagram, options).0
}

/// Convertit le SVG généré vers la sortie terminal selon le protocole choisi en extrayant le payload brut.
fn render_svg_to_terminal_with_raw(
    svg: &str,
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> (String, bool, String) {
    if options.protocol == GraphicsProtocol::Raw {
        let raw = format!("{svg}\n");
        return (raw.clone(), true, raw);
    }

    let target_cols = options.viewport.columns.max(10);
    let target_width_px = u32::from(target_cols) * 8;

    match rasterizer::rasterize_svg(svg, target_width_px, options.limits.max_raster_pixels) {
        Ok(image) => {
            let image = composite_on_theme_background(image, options.theme);
            let encoded = encode_image_for_protocol(&image, options);
            let title_prefix = diagram
                .title()
                .map_or_else(String::new, |t| format_title_header(t, target_cols));
            (format!("{title_prefix}{encoded}"), true, encoded)
        }
        Err(err) => (format_fallback(diagram, &err), false, String::new()),
    }
}

fn format_title_header(title: &str, target_cols: u16) -> String {
    let visual_len = ViewportGeometry::display_width(title);
    let remaining = usize::from(target_cols).saturating_sub(visual_len.saturating_add(2));
    let side = (remaining / 2).max(3);
    let border = "─".repeat(side);
    format!("\x1b[1m{border} {title} {border}\x1b[0m\n")
}

/// Rend les zones transparentes indépendantes du thème réel du terminal.
#[must_use]
fn composite_on_theme_background(mut image: RasterizedImage, theme: ThemeMode) -> RasterizedImage {
    let (bg_r, bg_g, bg_b) = theme.background_rgb();

    let (chunks, _) = image.rgba.as_chunks_mut::<4>();
    for pixel in chunks {
        let alpha = u16::from(pixel[3]);
        let inv_alpha = 255_u16.saturating_sub(alpha);
        pixel[0] = blend_channel(pixel[0], alpha, bg_r, inv_alpha);
        pixel[1] = blend_channel(pixel[1], alpha, bg_g, inv_alpha);
        pixel[2] = blend_channel(pixel[2], alpha, bg_b, inv_alpha);
        pixel[3] = 255;
    }

    image
}

fn blend_channel(foreground: u8, alpha: u16, background: u8, inv_alpha: u16) -> u8 {
    let blended = u16::from(foreground) * alpha + u16::from(background) * inv_alpha;
    u8::try_from(blended / 255).unwrap_or(0)
}

/// Encode l'image matricielle selon le protocole graphique actif.
fn encode_image_for_protocol(image: &RasterizedImage, options: RenderOptions) -> String {
    let target_cols = options.viewport.columns.max(10);
    match options.protocol {
        GraphicsProtocol::Kitty => kitty::encode_kitty_graphics(image),
        GraphicsProtocol::Iterm2 => iterm2::encode_iterm2(image).unwrap_or_default(),
        GraphicsProtocol::HalfBlocks => halfblock::encode_halfblocks(image, target_cols),
        GraphicsProtocol::AsciiBox => asciibox::encode_asciibox(image, target_cols),
        GraphicsProtocol::Raw => String::new(),
    }
}

/// Formate un bloc de repli gracieux en cas d'échec de rendu.
fn format_fallback(diagram: &DiagramBlock, err: &CliError) -> String {
    let title_prefix = diagram
        .title()
        .map_or_else(String::new, |t| format!("\x1b[1m─── {t} ───\x1b[0m\n"));
    format!(
        "{title_prefix}\x1b[33m⚠️  [Rendu Mermaid indisponible: {err}]\x1b[0m\n```mermaid\n{}\n```\n",
        diagram.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_diagram_valid_halfblocks() {
        let block = DiagramBlock::new("graph LR\n  A --> B".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let output = render_diagram(&block, options);
        assert_ne!(output, "");
        assert!(!output.contains("Rendu Mermaid indisponible"));
    }

    #[test]
    fn test_render_diagram_valid_asciibox() {
        let block = DiagramBlock::new("graph LR\n  A --> B".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::AsciiBox,
            ViewportGeometry::new(80, 24),
        );
        let output = render_diagram(&block, options);
        assert_ne!(output, "");
        assert!(!output.contains("Rendu Mermaid indisponible"));
        assert!(output.contains('A'));
        assert!(output.contains('B'));
        assert!(output.chars().any(|c| c == '┌' || c == '│' || c == '└'));
    }

    #[test]
    fn test_render_diagram_asciibox_state_diagram() {
        let block =
            DiagramBlock::new("stateDiagram-v2\n  [*] --> Still\n  Still --> [*]".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::AsciiBox,
            ViewportGeometry::new(80, 24),
        );
        let (output, is_valid) = render_diagram_checked(&block, options);
        assert!(is_valid);
        assert!(output.contains("Still"));
        assert!(output.chars().any(|c| c == '┌' || c == '│' || c == '└'));
    }

    #[test]
    fn test_render_diagram_invalid_syntax_fallback() {
        let block = DiagramBlock::new("xyz invalid $$$".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let output = render_diagram(&block, options);
        assert!(output.contains("Rendu Mermaid indisponible"));
        assert!(output.contains("```mermaid\nxyz invalid $$$"));
    }

    #[test]
    fn test_composite_on_theme_background_removes_transparency() {
        let image = RasterizedImage::new(1, 1, vec![255, 255, 255, 0]);
        let composed = composite_on_theme_background(image, ThemeMode::Dark);
        assert_eq!(composed.rgba, vec![30, 30, 30, 255]);

        let image_amber = RasterizedImage::new(1, 1, vec![255, 255, 255, 0]);
        let composed_amber = composite_on_theme_background(image_amber, ThemeMode::Amber);
        assert_eq!(composed_amber.rgba, vec![18, 16, 8, 255]);
    }

    #[test]
    fn test_analyze_and_render_diagram_valid() {
        let block = DiagramBlock::new("graph LR\n  A --> B".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let item = analyze_and_render_diagram(&block, options, 0);
        assert!(matches!(item, JsonStreamItem::Diagram { .. }));
        if let JsonStreamItem::Diagram {
            index,
            valid,
            title,
            dimensions,
            protocol,
            payload,
            error,
            raw_content,
        } = item
        {
            assert_eq!(index, 0);
            assert!(valid);
            assert!(title.is_none());
            assert!(dimensions.is_some());
            assert_eq!(protocol, Some(GraphicsProtocol::HalfBlocks));
            assert!(payload.is_some());
            assert!(error.is_none());
            assert_eq!(raw_content, "graph LR\n  A --> B");
        }
    }

    #[test]
    fn test_analyze_and_render_diagram_invalid_reports_line_and_error() {
        let block = DiagramBlock::new("graph TD\n  syntax error $$$".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let item = analyze_and_render_diagram(&block, options, 3);
        assert!(matches!(item, JsonStreamItem::Diagram { .. }));
        if let JsonStreamItem::Diagram {
            index,
            valid,
            title,
            dimensions,
            protocol,
            payload,
            error,
            raw_content,
        } = item
        {
            assert_eq!(index, 3);
            assert!(!valid);
            assert!(title.is_none());
            assert!(dimensions.is_none());
            assert!(protocol.is_none());
            assert!(payload.is_none());
            assert!(error.is_some());
            if let Some(err) = error {
                assert_eq!(err.line, Some(2));
            }
            assert_eq!(raw_content, "graph TD\n  syntax error $$$");
        }
    }

    #[test]
    fn test_render_diagram_valid_iterm2() {
        let block = DiagramBlock::new("graph LR\n  A --> B".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::Iterm2,
            ViewportGeometry::new(80, 24),
        );
        let output = render_diagram(&block, options);
        assert_ne!(output, "");
        assert!(
            output.starts_with(
                "\x1b]1337;File=inline=1;width=auto;height=auto;preserveAspectRatio=1:"
            )
        );
        assert!(output.ends_with("\x07\n"));
    }

    #[test]
    fn test_render_diagram_with_title_and_theme_override() {
        let raw =
            "```mermaid title=\"Architecture Cible\" theme=neon width=50\ngraph LR\n  A --> B\n```";
        let block = DiagramBlock::from_raw(raw);
        let options = RenderOptions::new(
            ThemeMode::Light, // Devrait être écrasé par neon
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let output = render_diagram(&block, options);
        assert!(output.contains("Architecture Cible"));
        assert!(!output.contains("Rendu Mermaid indisponible"));
    }

    #[test]
    fn test_render_diagram_fallback_with_title() {
        let raw = "```mermaid title=\"Erreur Syntaxique\"\ngraph LR\n  A --> $$$$%\n```";
        let block = DiagramBlock::from_raw(raw);
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let (output, valid) = render_diagram_checked(&block, options);
        assert!(!valid);
        assert!(output.contains("Erreur Syntaxique"));
        assert!(output.contains("Rendu Mermaid indisponible"));
    }

    #[test]
    fn test_render_diagram_hits_cache() {
        let block = DiagramBlock::new("graph TD\n  CacheA --> CacheB".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );
        let key = RenderCacheKey::new(
            block.as_str(),
            options.theme,
            options.viewport.target_columns(),
            options.protocol,
            options.engine,
        );

        // Premier appel : remplit le cache
        let (output1, valid1) = render_diagram_checked(&block, options);
        assert!(valid1);
        assert!(RenderCache::global().get(&key).is_some());

        // Deuxième appel : servi depuis le cache
        let (output2, valid2) = render_diagram_checked(&block, options);
        assert!(valid2);
        assert_eq!(output1, output2);
    }

    #[test]
    fn test_render_diagram_auto_orients_narrow_viewport() {
        let block = DiagramBlock::new("graph LR\n  Alpha --> Beta".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::AsciiBox,
            ViewportGeometry::new(60, 24),
        );
        let (output, valid) = render_diagram_checked(&block, options);
        assert!(valid);
        assert!(output.contains("Alpha"));
        assert!(output.contains("Beta"));
        let alpha_line = output
            .lines()
            .position(|l| l.contains("Alpha"))
            .unwrap_or(0);
        let beta_line = output.lines().position(|l| l.contains("Beta")).unwrap_or(0);
        assert!(alpha_line < beta_line);
    }

    #[test]
    fn test_render_diagram_respects_no_auto_orient() {
        let block = DiagramBlock::new("graph LR\n  Alpha --> Beta".to_string());
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::AsciiBox,
            ViewportGeometry::new(60, 24),
        )
        .with_auto_orient(false);
        let (output, valid) = render_diagram_checked(&block, options);
        assert!(valid);
        assert!(output.contains("Alpha"));
        assert!(output.contains("Beta"));
        let line_with_both = output
            .lines()
            .any(|l| l.contains("Alpha") && l.contains("Beta"));
        assert!(line_with_both);
    }

    #[cfg(feature = "merman")]
    #[test]
    fn test_render_diagram_with_merman_engine() {
        let block = DiagramBlock::new(
            "sequenceDiagram\n  autonumber\n  Alice->>Bob: Ping\n  Bob-->>Alice: Pong".to_string(),
        );
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        )
        .with_engine(DiagramEngineType::Merman);
        let (output, valid) = render_diagram_checked(&block, options);
        assert!(
            valid,
            "Rendering with merman should succeed for sequence diagram"
        );
        assert_ne!(output, "");
    }

    #[test]
    fn test_render_diagram_timeout_triggers_fallback() {
        let block = DiagramBlock::new("graph TD\n  A --> B --> C".to_string());
        let limits =
            ResourceLimits::default().with_render_timeout(Some(std::time::Duration::from_nanos(1)));
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let (output, valid) = render_diagram_checked(&block, options);
        assert!(!valid, "Must trigger fallback on timeout");
        assert!(output.contains("délai de rendu Mermaid dépassé"));
    }

    #[test]
    fn test_analyze_and_render_diagram_timeout_reports_error() {
        let block = DiagramBlock::new("graph TD\n  A --> B".to_string());
        let limits =
            ResourceLimits::default().with_render_timeout(Some(std::time::Duration::from_nanos(1)));
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let item = analyze_and_render_diagram(&block, options, 0);
        match item {
            JsonStreamItem::Diagram { valid, error, .. } => {
                assert!(!valid);
                let err = error.unwrap_or_else(|| unreachable!());
                assert_eq!(err.kind, Some("RenderTimeout".to_string()));
            }
            JsonStreamItem::Text { .. } => unreachable!(),
        }
    }
}
