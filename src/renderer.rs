use crate::cache::{RenderCache, RenderCacheKey};
use crate::domain::{
    CliError, DiagramBlock, DiagramDimensions, DiagramEngineType, DiagramErrorDetail,
    GraphicsProtocol, JsonStreamItem, OutputFormat, RasterizedImage, ResourceLimits, ThemeMode,
    ViewportGeometry, sanitize_terminal_text,
};
use crate::mermaid;
use crate::protocol::{asciibox, halfblock, iterm2, kitty};
use crate::rasterizer;

use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};

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
    pub fallback_asciibox: bool,
    /// Relaye le texte Markdown hors diagramme sans neutraliser ses séquences terminales.
    pub raw_passthrough: bool,
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
            fallback_asciibox: true,
            raw_passthrough: false,
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
            fallback_asciibox: true,
            raw_passthrough: false,
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
            fallback_asciibox: true,
            raw_passthrough: false,
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

    #[must_use]
    pub const fn with_fallback_asciibox(mut self, fallback_asciibox: bool) -> Self {
        self.fallback_asciibox = fallback_asciibox;
        self
    }

    #[must_use]
    pub const fn with_raw_passthrough(mut self, raw_passthrough: bool) -> Self {
        self.raw_passthrough = raw_passthrough;
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
        viewport,
        ..options
    }
}

fn prepare_diagram_for_rendering(
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> Cow<'_, DiagramBlock> {
    let sanitized = mermaid::sanitize_mermaid_labels(diagram.as_str());
    let target_cols = options.viewport.columns.max(10);
    let adapted = if options.auto_orient {
        mermaid::adapt_direction_for_viewport(&sanitized, target_cols)
    } else {
        Cow::Borrowed(sanitized.as_ref())
    };

    if let Cow::Owned(content) = adapted {
        Cow::Owned(DiagramBlock::with_metadata(
            content,
            diagram.metadata().clone(),
        ))
    } else if let Cow::Owned(content) = sanitized {
        Cow::Owned(DiagramBlock::with_metadata(
            content,
            diagram.metadata().clone(),
        ))
    } else {
        Cow::Borrowed(diagram)
    }
}

fn oversized_message(skipped_bytes: usize, limits: ResourceLimits) -> String {
    format!(
        "Diagramme Mermaid ignoré : {skipped_bytes} octets dépassent la limite de {} octets",
        limits.max_diagram_bytes
    )
}

/// Avertissement `GracefulFallback` d'un diagramme hors quota, sans reproduction du contenu.
#[must_use]
pub fn render_oversized_notice(skipped_bytes: usize, options: RenderOptions) -> String {
    format!(
        "\x1b[33m⚠️  [{}]\x1b[0m\n",
        oversized_message(skipped_bytes, options.limits)
    )
}

/// Élément structuré d'un diagramme hors quota (`valid: false`, `kind: ResourceLimit`).
#[must_use]
pub fn oversized_diagram_json_item(
    skipped_bytes: usize,
    options: RenderOptions,
    index: usize,
) -> JsonStreamItem {
    JsonStreamItem::Diagram {
        index,
        valid: false,
        title: None,
        dimensions: None,
        protocol: None,
        payload: None,
        error: Some(DiagramErrorDetail::new(
            oversized_message(skipped_bytes, options.limits),
            None,
            Some("ResourceLimit".to_string()),
        )),
        raw_content: String::new(),
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
    let limits = effective_options.limits;

    let res = run_with_render_timeout(
        move || {
            let item = if effective_options.protocol == GraphicsProtocol::AsciiBox {
                render_asciibox_to_json_item(&render_target, effective_options, index)
            } else {
                render_svg_or_fallback_json_item(&render_target, effective_options, index)
            };
            Ok(item)
        },
        &limits,
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

fn render_svg_or_fallback_json_item(
    render_target: &DiagramBlock,
    options: RenderOptions,
    index: usize,
) -> JsonStreamItem {
    match mermaid::render_to_svg_detailed_with_engine(render_target, options.theme, options.engine)
    {
        Ok(svg) => {
            let item = render_svg_to_json_item(&svg, render_target, options, index);
            if matches!(item, JsonStreamItem::Diagram { valid: false, .. })
                && options.fallback_asciibox
            {
                let ascii_item = render_asciibox_to_json_item(render_target, options, index);
                if matches!(ascii_item, JsonStreamItem::Diagram { valid: true, .. }) {
                    return ascii_item;
                }
            }
            item
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
    let encoded = rasterizer::rasterize_svg(svg, target_width_px, options.limits.max_raster_pixels)
        .and_then(|image| {
            let dimensions = DiagramDimensions::new(image.width, image.height);
            let image = composite_on_theme_background(image, options.theme);
            encode_image_for_protocol(&image, options).map(|payload| (dimensions, payload))
        });
    match encoded {
        Ok((dimensions, payload)) => JsonStreamItem::Diagram {
            index,
            valid: true,
            title: diagram.title().map(ToString::to_string),
            dimensions: Some(dimensions),
            protocol: Some(options.protocol),
            payload: Some(payload),
            error: None,
            raw_content: diagram.as_str().to_string(),
        },
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
                Some(raster_error_kind(&err).to_string()),
            )),
            raw_content: diagram.as_str().to_string(),
        },
    }
}

/// Cycle de vie d'un rendu sous garde temporelle, partagé entre l'appelant et le
/// thread de rendu pour décider sans course lequel des deux libère le compteur.
const RENDER_RUNNING: u8 = 0;
const RENDER_FINISHED: u8 = 1;
const RENDER_ORPHANED: u8 = 2;

/// Compteur des rendus orphelins : abandonnés après expiration du délai mais toujours
/// en cours, `mermaid-svg` n'offrant aucune annulation.
#[derive(Debug, Default)]
pub(crate) struct RenderThreadBudget {
    orphans: Arc<AtomicUsize>,
}

/// Garde exécutée à la fin du thread de rendu, y compris lors d'un panic du moteur :
/// libère le compteur si l'appelant a abandonné le rendu entre-temps.
struct RenderCompletion {
    state: Arc<AtomicU8>,
    orphans: Arc<AtomicUsize>,
}

impl Drop for RenderCompletion {
    fn drop(&mut self) {
        let finished_in_time = self
            .state
            .compare_exchange(
                RENDER_RUNNING,
                RENDER_FINISHED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok();
        if !finished_in_time {
            self.orphans.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

impl RenderThreadBudget {
    fn orphans(&self) -> usize {
        self.orphans.load(Ordering::SeqCst)
    }

    /// Marque le rendu comme orphelin ; le compteur est incrémenté avant la transition
    /// afin que le thread ne puisse jamais le décrémenter en premier.
    fn abandon(&self, state: &AtomicU8) {
        self.orphans.fetch_add(1, Ordering::SeqCst);
        let abandoned = state
            .compare_exchange(
                RENDER_RUNNING,
                RENDER_ORPHANED,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_ok();
        if !abandoned {
            self.orphans.fetch_sub(1, Ordering::SeqCst);
        }
    }

    /// Exécute `f` dans un thread dédié sous la garde de `limits.render_timeout` ;
    /// refuse immédiatement si `limits.max_orphan_renders` rendus orphelins tournent.
    pub(crate) fn run_with_timeout<F, T>(
        &self,
        f: F,
        limits: &ResourceLimits,
    ) -> Result<T, CliError>
    where
        F: FnOnce() -> Result<T, CliError> + Send + 'static,
        T: Send + 'static,
    {
        let Some(timeout_dur) = limits.render_timeout else {
            return f();
        };
        if self.orphans() >= limits.max_orphan_renders {
            return Err(CliError::ResourceLimit(format!(
                "trop de rendus abandonnés encore en cours ({} au maximum)",
                limits.max_orphan_renders
            )));
        }

        let state = Arc::new(AtomicU8::new(RENDER_RUNNING));
        let completion = RenderCompletion {
            state: Arc::clone(&state),
            orphans: Arc::clone(&self.orphans),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _completion = completion;
            let _ = tx.send(f());
        });

        match rx.recv_timeout(timeout_dur) {
            Ok(res) => res,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                self.abandon(&state);
                Err(CliError::ResourceLimit(format!(
                    "délai de rendu Mermaid dépassé ({} ms)",
                    timeout_dur.as_millis()
                )))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(CliError::ResourceLimit(
                "interruption anormale du thread de rendu Mermaid".to_string(),
            )),
        }
    }
}

static RENDER_THREADS: LazyLock<RenderThreadBudget> = LazyLock::new(RenderThreadBudget::default);

/// Exécute `f` sous garde temporelle avec le budget global de rendus orphelins.
pub(crate) fn run_with_render_timeout<F, T>(f: F, limits: &ResourceLimits) -> Result<T, CliError>
where
    F: FnOnce() -> Result<T, CliError> + Send + 'static,
    T: Send + 'static,
{
    RENDER_THREADS.run_with_timeout(f, limits)
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
    let limits = effective_options.limits;

    let render_result = run_with_render_timeout(
        move || {
            if effective_options.protocol == GraphicsProtocol::AsciiBox {
                Ok(render_asciibox_to_terminal(
                    &diag_for_render,
                    effective_options,
                ))
            } else {
                Ok(render_graphical_with_fallback(
                    &diag_for_render,
                    effective_options,
                ))
            }
        },
        &limits,
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

fn render_asciibox_fallback(
    diagram: &DiagramBlock,
    options: RenderOptions,
    original_err: &CliError,
) -> (String, bool, String) {
    let target_cols = options.viewport.columns.max(10);
    let ascii_opts = asciibox::AsciiBoxOptions::new(target_cols, options.theme != ThemeMode::Mono);
    match asciibox::render_asciibox(diagram.as_str(), ascii_opts) {
        Ok(text) => {
            let title_prefix = diagram
                .title()
                .map_or_else(String::new, |t| format_title_header(t, target_cols));
            let error_text = original_err.to_string();
            let warning = format!(
                "\x1b[33m⚠️  [Rendu graphique indisponible: {}; repli automatique en mode AsciiBox]\x1b[0m\n",
                sanitize_terminal_text(&error_text)
            );
            (format!("{warning}{title_prefix}{text}"), true, text)
        }
        Err(_) => (format_fallback(diagram, original_err), false, String::new()),
    }
}

fn render_graphical_with_fallback(
    diagram: &DiagramBlock,
    options: RenderOptions,
) -> (String, bool, String) {
    let render_result = mermaid::render_to_svg_with_engine(diagram, options.theme, options.engine)
        .and_then(|svg| render_svg_to_terminal_with_raw(&svg, diagram, options));

    match render_result {
        Ok((output, raw_payload)) => (output, true, raw_payload),
        Err(err) => {
            if options.fallback_asciibox && !matches!(err, CliError::MermaidSyntax(_)) {
                render_asciibox_fallback(diagram, options, &err)
            } else {
                (format_fallback(diagram, &err), false, String::new())
            }
        }
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
) -> Result<(String, String), CliError> {
    if options.protocol == GraphicsProtocol::Raw {
        let raw = format!("{svg}\n");
        return Ok((raw.clone(), raw));
    }

    let target_cols = options.viewport.columns.max(10);
    let target_width_px = u32::from(target_cols) * 8;

    let image = rasterizer::rasterize_svg(svg, target_width_px, options.limits.max_raster_pixels)?;
    let image = composite_on_theme_background(image, options.theme);
    let encoded = encode_image_for_protocol(&image, options)?;
    let title_prefix = diagram
        .title()
        .map_or_else(String::new, |t| format_title_header(t, target_cols));
    Ok((format!("{title_prefix}{encoded}"), encoded))
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

/// Catégorie d'erreur exposée en JSON pour un échec après génération du SVG.
const fn raster_error_kind(err: &CliError) -> &'static str {
    if matches!(err, CliError::ImageEncoding(_)) {
        "ImageEncodingError"
    } else {
        "RasterizeError"
    }
}

/// Encode l'image matricielle selon le protocole graphique actif.
///
/// # Errors
/// Renvoie `CliError::ImageEncoding` si l'encodage PNG requis par iTerm2 échoue,
/// afin de déclencher le repli au lieu d'émettre une image vide.
fn encode_image_for_protocol(
    image: &RasterizedImage,
    options: RenderOptions,
) -> Result<String, CliError> {
    let target_cols = options.viewport.columns.max(10);
    Ok(match options.protocol {
        GraphicsProtocol::Kitty => kitty::encode_kitty_graphics(image),
        GraphicsProtocol::Iterm2 => iterm2::encode_iterm2(image)?,
        GraphicsProtocol::HalfBlocks => halfblock::encode_halfblocks(image, target_cols),
        GraphicsProtocol::AsciiBox => asciibox::encode_asciibox(image, target_cols),
        GraphicsProtocol::Raw => String::new(),
    })
}

/// Formate un bloc de repli gracieux en cas d'échec de rendu.
///
/// Le contenu et le message d'erreur, non fiables, sont neutralisés avant réémission.
fn format_fallback(diagram: &DiagramBlock, err: &CliError) -> String {
    let title_prefix = diagram
        .title()
        .map_or_else(String::new, |t| format!("\x1b[1m─── {t} ───\x1b[0m\n"));
    let error_text = err.to_string();
    let source = sanitize_terminal_text(diagram.as_str());
    let fence = markdown_fence_for(&source);
    format!(
        "{title_prefix}\x1b[33m⚠️  [Rendu Mermaid indisponible: {}]\x1b[0m\n{fence}mermaid\n{source}\n{fence}\n",
        sanitize_terminal_text(&error_text)
    )
}

/// Clôture Markdown plus longue que toute suite de backticks du contenu (`CommonMark`),
/// afin que le contenu réémis ne puisse pas fermer le bloc de repli.
fn markdown_fence_for(content: &str) -> String {
    let longest_run = content.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest_run.saturating_add(1).max(3))
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
    fn test_render_diagram_fallback_neutralizes_terminal_sequences() {
        let block = DiagramBlock::new(
            "xyz \x1b]0;PWNED\x07 \x1b]52;c;ZXZpbA==\x07 \x1b_Ga=T;AAAA\x1b\\ $$$".to_string(),
        );
        let output = render_diagram(&block, RenderOptions::default());
        assert!(output.contains("Rendu Mermaid indisponible"));
        assert!(!output.contains("\x1b]"), "OSC résiduel : {output:?}");
        assert!(!output.contains("\x1b_"), "APC résiduel : {output:?}");
        assert!(output.contains("xyz"));
    }

    #[test]
    fn test_format_fallback_fence_outlasts_content_backticks() {
        let block = DiagramBlock::new("A\n```\nB ````` C".to_string());
        let output = format_fallback(&block, &CliError::MermaidSyntax("x".to_string()));
        assert!(output.contains("\n``````mermaid\nA\n```\nB ````` C\n``````\n"));
    }

    #[test]
    fn test_format_fallback_neutralizes_error_message() {
        let block = DiagramBlock::new("graph".to_string());
        let err = CliError::MermaidSyntax("label \x1b]0;PWNED\x07".to_string());
        assert!(!format_fallback(&block, &err).contains("\x1b]"));
    }

    fn budget_limits(max_orphan_renders: usize) -> ResourceLimits {
        ResourceLimits {
            max_orphan_renders,
            render_timeout: Some(std::time::Duration::from_millis(10)),
            ..ResourceLimits::default()
        }
    }

    fn sleeping_render(millis: u64) -> impl FnOnce() -> Result<(), CliError> + Send + 'static {
        move || {
            std::thread::sleep(std::time::Duration::from_millis(millis));
            Ok(())
        }
    }

    fn wait_until_no_orphan(budget: &RenderThreadBudget) -> usize {
        for _ in 0..200 {
            if budget.orphans() == 0 {
                return 0;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        budget.orphans()
    }

    #[test]
    fn test_render_budget_caps_orphan_renders() {
        let budget = RenderThreadBudget::default();
        let limits = budget_limits(4);
        let outcomes: Vec<Result<(), CliError>> = (0..20)
            .map(|_| budget.run_with_timeout(sleeping_render(200), &limits))
            .collect();

        assert!(budget.orphans() <= 4, "orphelins : {}", budget.orphans());
        let refused = outcomes
            .iter()
            .filter(|res| matches!(res, Err(CliError::ResourceLimit(msg)) if msg.contains("abandonnés")))
            .count();
        assert_eq!(refused, 16);
        assert_eq!(wait_until_no_orphan(&budget), 0);
        assert_eq!(budget.run_with_timeout(|| Ok(1), &limits), Ok(1));
    }

    #[test]
    fn test_render_budget_does_not_count_renders_finished_in_time() {
        let budget = RenderThreadBudget::default();
        let limits = ResourceLimits {
            max_orphan_renders: 1,
            ..ResourceLimits::default()
        };
        for _ in 0..10 {
            assert_eq!(budget.run_with_timeout(|| Ok(()), &limits), Ok(()));
        }
        assert_eq!(budget.orphans(), 0);
    }

    #[test]
    fn test_render_budget_releases_orphan_when_render_panics() {
        let budget = RenderThreadBudget::default();
        let limits = budget_limits(1);
        let res: Result<(), CliError> = budget.run_with_timeout(
            || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                std::panic::resume_unwind(Box::new("moteur en échec"))
            },
            &limits,
        );
        assert!(matches!(res, Err(CliError::ResourceLimit(_))));
        assert_eq!(wait_until_no_orphan(&budget), 0);
        assert_eq!(budget.run_with_timeout(|| Ok(7), &limits), Ok(7));
    }

    #[test]
    fn test_render_budget_without_timeout_runs_inline() {
        let budget = RenderThreadBudget::default();
        let limits = ResourceLimits::default().with_render_timeout(None);
        assert_eq!(
            budget.run_with_timeout(|| Ok("inline"), &limits),
            Ok("inline")
        );
        assert_eq!(budget.orphans(), 0);
    }

    #[test]
    fn test_encode_image_for_protocol_reports_iterm2_failure() {
        let truncated = RasterizedImage::new(4, 4, vec![0; 10]);
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::Iterm2,
            ViewportGeometry::new(80, 24),
        );
        assert!(matches!(
            encode_image_for_protocol(&truncated, options),
            Err(CliError::ImageEncoding(_))
        ));
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

    #[test]
    fn test_render_diagram_raster_limit_triggers_asciibox_fallback() {
        let block = DiagramBlock::new("graph TD\n  RasterLimitA --> RasterLimitB".to_string());
        let limits = ResourceLimits {
            max_raster_pixels: 1,
            ..ResourceLimits::default()
        };
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let (output, valid) = render_diagram_checked(&block, options);
        assert!(valid, "Fallback to AsciiBox must be considered valid");
        assert!(output.contains("repli automatique en mode AsciiBox"));
        assert!(output.contains("RasterLimitA"));
        assert!(output.contains("RasterLimitB"));
    }

    #[test]
    fn test_render_diagram_raster_limit_with_no_fallback_asciibox() {
        let block = DiagramBlock::new("graph TD\n  NoFallbackA --> NoFallbackB".to_string());
        let limits = ResourceLimits {
            max_raster_pixels: 1,
            ..ResourceLimits::default()
        };
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        )
        .with_fallback_asciibox(false);

        let (output, valid) = render_diagram_checked(&block, options);
        assert!(!valid, "Must not be valid without AsciiBox fallback");
        assert!(output.contains("Rendu Mermaid indisponible"));
        assert!(output.contains("NoFallbackA --> NoFallbackB"));
    }

    #[test]
    fn test_analyze_and_render_diagram_raster_limit_triggers_asciibox_fallback() {
        let block = DiagramBlock::new("graph TD\n  JsonFallbackA --> JsonFallbackB".to_string());
        let limits = ResourceLimits {
            max_raster_pixels: 1,
            ..ResourceLimits::default()
        };
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        let item = analyze_and_render_diagram(&block, options, 0);
        match item {
            JsonStreamItem::Diagram {
                valid, protocol, ..
            } => {
                assert!(valid);
                assert_eq!(protocol, Some(GraphicsProtocol::AsciiBox));
            }
            JsonStreamItem::Text { .. } => unreachable!(),
        }
    }
}
