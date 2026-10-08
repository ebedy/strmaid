use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::time::Duration;
use unicode_width::UnicodeWidthChar;

/// Thème visuel pour le rendu SVG Mermaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "lower")]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    Neutral,
    Amber,
    Phosphor,
    Neon,
    Mono,
}

impl ThemeMode {
    #[must_use]
    pub fn from_str_name(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "light" => Self::Light,
            "neutral" => Self::Neutral,
            "amber" | "retro-amber" => Self::Amber,
            "phosphor" | "retro-phosphor" => Self::Phosphor,
            "neon" | "retro-neon" => Self::Neon,
            "mono" | "retro-mono" => Self::Mono,
            _ => Self::Dark,
        }
    }

    /// Triplet RVB de la couleur d'arrière-plan du thème pour la composition terminale.
    #[must_use]
    pub const fn background_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Dark => (30, 30, 30),
            Self::Light | Self::Neutral => (255, 255, 255),
            Self::Amber => (18, 16, 8),
            Self::Phosphor => (10, 20, 10),
            Self::Neon => (13, 11, 24),
            Self::Mono => (0, 0, 0),
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Neutral => "neutral",
            Self::Amber => "amber",
            Self::Phosphor => "phosphor",
            Self::Neon => "neon",
            Self::Mono => "mono",
        }
    }
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Protocole graphique terminal supporté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "lower")]
#[serde(rename_all = "lowercase")]
pub enum GraphicsProtocol {
    #[default]
    Kitty,
    #[value(name = "iterm2", alias = "iterm")]
    Iterm2,
    #[value(name = "halfblocks", alias = "halfblock")]
    HalfBlocks,
    #[value(name = "asciibox", alias = "ascii")]
    AsciiBox,
    Raw,
}

/// Format de sortie des flux analysés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "lower")]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
    Ndjson,
}

/// Moteur de génération vectorielle SVG pour diagrammes Mermaid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, ValueEnum, Serialize, Deserialize)]
#[value(rename_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum DiagramEngineType {
    #[default]
    MermaidSvg,
    Merman,
}

impl DiagramEngineType {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::MermaidSvg => "mermaid-svg",
            Self::Merman => "merman",
        }
    }
}

impl std::fmt::Display for DiagramEngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Dimensions matricielles d'un diagramme rendu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramDimensions {
    pub width: u32,
    pub height: u32,
}

impl DiagramDimensions {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// Détail d'une erreur de syntaxe ou de rendu Mermaid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramErrorDetail {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl DiagramErrorDetail {
    #[must_use]
    pub fn new(message: String, line: Option<usize>, kind: Option<String>) -> Self {
        Self {
            message,
            line,
            kind,
        }
    }
}

/// Élément structuré émis dans les formats machine-readable (JSON / NDJSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum JsonStreamItem {
    Text {
        content: String,
    },
    Diagram {
        index: usize,
        valid: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        dimensions: Option<DiagramDimensions>,
        #[serde(skip_serializing_if = "Option::is_none")]
        protocol: Option<GraphicsProtocol>,
        #[serde(skip_serializing_if = "Option::is_none")]
        payload: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<DiagramErrorDetail>,
        raw_content: String,
    },
}

/// Synthèse quantitative du document pour la sortie globale JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct JsonStreamSummary {
    pub total_items: usize,
    pub total_diagrams: usize,
    pub valid_diagrams: usize,
    pub invalid_diagrams: usize,
}

/// Document JSON complet émis à l'EOF en mode `--format json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonDocumentOutput {
    pub version: String,
    pub format: OutputFormat,
    pub theme: ThemeMode,
    pub protocol: GraphicsProtocol,
    pub items: Vec<JsonStreamItem>,
    pub summary: JsonStreamSummary,
}

impl GraphicsProtocol {
    #[must_use]
    pub fn from_str_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "kitty" => Some(Self::Kitty),
            "halfblock" | "halfblocks" => Some(Self::HalfBlocks),
            "ascii" | "asciibox" => Some(Self::AsciiBox),
            "raw" => Some(Self::Raw),
            _ => None,
        }
    }
}

/// Mode d'exécution de l'outil CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    StreamFilter,
    LivePager,
}

/// Limites de ressources appliquées aux entrées et rendus non fiables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_diagram_bytes: usize,
    pub max_raster_pixels: u32,
    pub max_pager_lines: usize,
    pub max_cached_diagrams: usize,
    pub render_timeout: Option<Duration>,
    /// Nombre maximal de rendus abandonnés après expiration de `render_timeout` et
    /// encore en cours (les moteurs ne sont pas interruptibles) ; au-delà, tout nouveau
    /// rendu sous garde temporelle est refusé.
    pub max_orphan_renders: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_diagram_bytes: 1_048_576,
            max_raster_pixels: 8_000_000,
            max_pager_lines: 20_000,
            max_cached_diagrams: 32,
            render_timeout: Some(Duration::from_millis(5000)),
            max_orphan_renders: 4,
        }
    }
}

impl ResourceLimits {
    #[must_use]
    pub const fn with_render_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.render_timeout = timeout;
        self
    }
}

/// Métadonnées extraites d'une balise fenced Mermaid (` ```mermaid title="..." theme=... width=... `).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DiagramMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_override: Option<ThemeMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_override: Option<u16>,
}

impl DiagramMetadata {
    /// Analyse de manière linéaire sans regex la ligne d'ouverture fenced Mermaid.
    ///
    /// Respecte une politique allow-list stricte (`title`, `theme`, `width`)
    /// et filtre tout caractère de contrôle ou séquence d'échappement terminal (`\x1b`).
    #[must_use]
    pub fn parse_fenced_info(info: &str) -> Self {
        let mut metadata = Self::default();
        let mut chars = info.chars().peekable();

        while let Some((key, val)) = parse_next_kv(&mut chars) {
            metadata.apply_attribute(&key, &val);
        }

        metadata
    }

    fn apply_attribute(&mut self, key: &str, val: &str) {
        match key {
            "title" => self.title = sanitize_title(val),
            "theme" => self.theme_override = Some(ThemeMode::from_str_name(val)),
            "width" => self.width_override = parse_positive_width(val),
            _ => {}
        }
    }
}

/// Caractère de clôture d'un bloc de code `CommonMark`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceMarker {
    Backtick,
    Tilde,
}

impl FenceMarker {
    const fn from_char(ch: char) -> Option<Self> {
        match ch {
            '`' => Some(Self::Backtick),
            '~' => Some(Self::Tilde),
            _ => None,
        }
    }

    const fn as_char(self) -> char {
        match self {
            Self::Backtick => '`',
            Self::Tilde => '~',
        }
    }
}

/// Clôture d'ouverture d'un bloc de code selon `CommonMark` : au moins trois `` ` `` ou
/// `~` identiques, suivis d'une info-string dont le premier mot est le langage.
///
/// Écart assumé : l'indentation n'est pas limitée à trois espaces, afin de reconnaître
/// les blocs imbriqués dans des listes sans analyser leur structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFence {
    marker: FenceMarker,
    length: usize,
    language: String,
    attributes: String,
}

impl CodeFence {
    /// Analyse une ligne d'ouverture ; `None` si la ligne n'ouvre aucun bloc de code.
    #[must_use]
    pub fn parse_opening(line: &str) -> Option<Self> {
        let trimmed = line.trim_start();
        let marker = FenceMarker::from_char(trimmed.chars().next()?)?;
        let length = marker_run_length(trimmed, marker);
        if length < 3 {
            return None;
        }
        let info = trimmed[length..].trim();
        if marker == FenceMarker::Backtick && info.contains('`') {
            return None;
        }
        let (language, attributes) = info.split_once(char::is_whitespace).unwrap_or((info, ""));
        Some(Self {
            marker,
            length,
            language: language.to_string(),
            attributes: attributes.to_string(),
        })
    }

    /// Indique si `line` ferme ce bloc : même caractère, longueur supérieure ou égale,
    /// aucune info-string.
    #[must_use]
    pub fn is_closed_by(&self, line: &str) -> bool {
        let trimmed = line.trim();
        let marker = self.marker.as_char();
        trimmed.len() >= self.length && trimmed.chars().all(|ch| ch == marker)
    }

    /// Bloc dont le langage désigne un diagramme Mermaid (`mermaid` ou `mermaidjs`).
    #[must_use]
    pub fn is_mermaid(&self) -> bool {
        matches!(self.language.as_str(), "mermaid" | "mermaidjs")
    }

    /// Métadonnées `title`, `theme` et `width` déclarées après le langage.
    #[must_use]
    pub fn metadata(&self) -> DiagramMetadata {
        DiagramMetadata::parse_fenced_info(&self.attributes)
    }
}

fn marker_run_length(text: &str, marker: FenceMarker) -> usize {
    let marker = marker.as_char();
    text.chars().take_while(|&ch| ch == marker).count()
}

fn parse_positive_width(val: &str) -> Option<u16> {
    val.parse::<u16>().ok().filter(|&w| w > 0)
}

fn sanitize_title(val: &str) -> Option<String> {
    let clean = sanitize_terminal_text(val).replace(['\n', '\t'], " ");
    let trimmed = clean.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn skip_whitespace(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while let Some(&c) = chars.peek() {
        if !c.is_whitespace() {
            break;
        }
        chars.next();
    }
}

fn read_quoted_value(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, quote: char) -> String {
    let mut val = String::new();
    for c in chars.by_ref() {
        if c == quote {
            break;
        }
        val.push(c);
    }
    val
}

fn read_unquoted_value(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut val = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            break;
        }
        val.push(c);
        chars.next();
    }
    val
}

fn parse_next_kv(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<(String, String)> {
    skip_whitespace(chars);
    let mut key = String::new();
    while let Some(&c) = chars.peek() {
        if c == '=' || c.is_whitespace() {
            break;
        }
        key.push(c);
        chars.next();
    }

    if key.is_empty() {
        return None;
    }

    skip_whitespace(chars);
    if chars.peek() != Some(&'=') {
        return Some((key.to_ascii_lowercase(), String::new()));
    }
    chars.next();
    skip_whitespace(chars);

    let val = match chars.peek() {
        Some(&quote @ ('"' | '\'')) => {
            chars.next();
            read_quoted_value(chars, quote)
        }
        _ => read_unquoted_value(chars),
    };

    Some((key.to_ascii_lowercase(), val))
}

/// Bloc de diagramme Mermaid capturé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramBlock {
    content: String,
    metadata: DiagramMetadata,
}

impl DiagramBlock {
    #[must_use]
    pub const fn new(content: String) -> Self {
        Self {
            content,
            metadata: DiagramMetadata {
                title: None,
                theme_override: None,
                width_override: None,
            },
        }
    }

    #[must_use]
    pub const fn with_metadata(content: String, metadata: DiagramMetadata) -> Self {
        Self { content, metadata }
    }

    /// Construit un bloc en éliminant les éventuelles sentinelles Markdown fences et extrait les métadonnées.
    #[must_use]
    pub fn from_raw(raw: &str) -> Self {
        let (content, metadata) = extract_mermaid_content_and_meta(raw);
        Self { content, metadata }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub const fn metadata(&self) -> &DiagramMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.metadata.title.as_deref()
    }

    #[must_use]
    pub const fn theme_override(&self) -> Option<ThemeMode> {
        self.metadata.theme_override
    }

    #[must_use]
    pub const fn width_override(&self) -> Option<u16> {
        self.metadata.width_override
    }
}

/// Extrait le contenu d'un bloc éventuellement entouré de fences, avec la même
/// grammaire que le streaming : métadonnées lues sur une fence Mermaid, contenu
/// jusqu'à la clôture correspondante. Sans fence d'ouverture, le texte est le contenu.
fn extract_mermaid_content_and_meta(input: &str) -> (String, DiagramMetadata) {
    let trimmed = input.trim();
    let (first_line, rest) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
    let Some(fence) = CodeFence::parse_opening(first_line) else {
        return (trimmed.to_string(), DiagramMetadata::default());
    };
    let metadata = if fence.is_mermaid() {
        fence.metadata()
    } else {
        DiagramMetadata::default()
    };
    let content: Vec<&str> = rest
        .lines()
        .take_while(|line| !fence.is_closed_by(line))
        .collect();
    (content.join("\n").trim().to_string(), metadata)
}

/// Élimine les sentinelles Markdown éventuelles entourant une spécification Mermaid.
#[must_use]
pub fn strip_mermaid_fences(input: &str) -> String {
    extract_mermaid_content_and_meta(input).0
}

/// Image matricielle RGBA prête pour l'affichage terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterizedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl RasterizedImage {
    #[must_use]
    pub const fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            rgba,
        }
    }
}

/// Géométrie du terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportGeometry {
    pub columns: u16,
    pub rows: u16,
}

impl ViewportGeometry {
    #[must_use]
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self { columns, rows }
    }

    /// Calcule le nombre de colonnes cibles pour le tracé matriciel en appliquant
    /// une compaction progressive sur les terminaux étroits (< 80 colonnes).
    #[must_use]
    pub fn target_columns(&self) -> u16 {
        let (numerator, denominator) = match self.columns {
            100.. => (9, 10),
            80..=99 => (93, 100),
            50..=79 => (96, 100),
            0..=49 => return self.columns,
        };
        let compacted = u32::from(self.columns) * numerator / denominator;
        u16::try_from(compacted).unwrap_or(u16::MAX)
    }

    /// Calcule la largeur d'affichage en colonnes d'une chaîne (compatible CJK & emojis).
    #[must_use]
    pub fn display_width(text: &str) -> usize {
        display_width(text)
    }

    /// Calcule la largeur CJK East Asian Ambiguous d'une chaîne.
    #[must_use]
    pub fn cjk_display_width(text: &str) -> usize {
        cjk_display_width(text)
    }

    /// Tronque une ligne pour qu'elle s'insère dans la largeur du viewport.
    #[must_use]
    pub fn fit_line(&self, text: &str) -> String {
        fit_to_width(text, usize::from(self.columns))
    }

    /// Complète une chaîne avec des espaces pour atteindre une largeur visuelle cible.
    #[must_use]
    pub fn pad_to_width(text: &str, target_cols: usize) -> String {
        pad_to_width(text, target_cols)
    }
}

/// Nature d'un segment de texte destiné au terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentKind {
    Char(char),
    /// Séquence CSI de style (`ESC [ … m`) aux paramètres strictement numériques.
    Sgr,
    /// Toute autre séquence d'échappement (CSI, OSC, DCS, APC, PM, SOS, ESC + 1 caractère).
    OtherEscape,
}

/// Segment de texte : un caractère ou une séquence d'échappement complète.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerminalSegment<'a> {
    raw: &'a str,
    kind: SegmentKind,
}

/// Politique de filtrage des séquences d'échappement lors de l'assainissement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EscapePolicy {
    StripAll,
    KeepSgr,
}

impl EscapePolicy {
    const fn allows(self, kind: SegmentKind) -> bool {
        match (self, kind) {
            (_, SegmentKind::Char(ch)) => !is_unsafe_terminal_char(ch),
            (Self::KeepSgr, SegmentKind::Sgr) => true,
            (Self::StripAll, SegmentKind::Sgr) | (_, SegmentKind::OtherEscape) => false,
        }
    }
}

/// Caractère de contrôle C0, DEL ou C1 interprétable par un terminal (hors `\n` et `\t`).
const fn is_unsafe_terminal_char(ch: char) -> bool {
    ch.is_control() && ch != '\n' && ch != '\t'
}

/// Découpe un texte en caractères et séquences d'échappement ECMA-48.
fn terminal_segments(text: &str) -> impl Iterator<Item = TerminalSegment<'_>> {
    let mut rest = text;
    std::iter::from_fn(move || {
        let ch = rest.chars().next()?;
        let (len, kind) = if ch == '\x1b' {
            escape_sequence_len(rest)
        } else {
            (ch.len_utf8(), SegmentKind::Char(ch))
        };
        let (raw, tail) = rest.split_at(len);
        rest = tail;
        Some(TerminalSegment { raw, kind })
    })
}

/// Longueur en octets de la séquence débutant par `ESC` en tête de `text`.
fn escape_sequence_len(text: &str) -> (usize, SegmentKind) {
    let body = &text[1..];
    let consumed = match body.chars().next() {
        Some('[') => return csi_sequence_len(&body[1..]),
        Some(']' | 'P' | '_' | '^' | 'X') => 1 + string_sequence_len(&body[1..]),
        Some(' '..='~') => 1,
        _ => 0,
    };
    (1 + consumed, SegmentKind::OtherEscape)
}

/// Longueur d'une séquence CSI (`ESC [` inclus) : paramètres et intermédiaires
/// `0x20..=0x3F`, puis octet final `0x40..=0x7E`. Une séquence malformée s'arrête
/// au premier caractère hors grammaire, qui n'est pas consommé.
fn csi_sequence_len(params: &str) -> (usize, SegmentKind) {
    let (end, final_char) = params
        .char_indices()
        .find(|&(_, c)| !(' '..='?').contains(&c))
        .map_or((params.len(), None), |(idx, c)| (idx, Some(c)));
    let final_len = usize::from(final_char.is_some_and(|c| ('@'..='~').contains(&c)));
    let is_sgr = final_char == Some('m')
        && params[..end]
            .chars()
            .all(|c| c.is_ascii_digit() || c == ';' || c == ':');
    let kind = if is_sgr {
        SegmentKind::Sgr
    } else {
        SegmentKind::OtherEscape
    };
    (2 + end + final_len, kind)
}

/// Longueur d'une chaîne de contrôle (OSC, DCS, APC, PM, SOS) jusqu'à son terminateur
/// `BEL`, `ESC \` ou `ST` (U+009C) inclus. Un `ESC` isolé l'interrompt sans être consommé ;
/// une chaîne non terminée consomme le reste du texte.
fn string_sequence_len(content: &str) -> usize {
    let Some((idx, terminator)) = content
        .char_indices()
        .find(|&(_, c)| matches!(c, '\x07' | '\x1b' | '\u{9c}'))
    else {
        return content.len();
    };
    match terminator {
        '\x1b' if content[idx + 1..].starts_with('\\') => idx + 2,
        '\x1b' => idx,
        other => idx + other.len_utf8(),
    }
}

fn sanitize_with_policy(text: &str, policy: EscapePolicy) -> Cow<'_, str> {
    if !text.chars().any(is_unsafe_terminal_char) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        terminal_segments(text)
            .filter(|segment| policy.allows(segment.kind))
            .map(|segment| segment.raw)
            .collect(),
    )
}

/// Neutralise toute séquence de contrôle terminal (CSI, OSC, DCS, APC, PM, SOS)
/// et tout caractère de contrôle C0/C1 hors `\n` et `\t`.
///
/// À appliquer à tout contenu non fiable réémis vers le terminal (replis, erreurs).
#[must_use]
pub fn sanitize_terminal_text(text: &str) -> Cow<'_, str> {
    sanitize_with_policy(text, EscapePolicy::StripAll)
}

/// Variante de [`sanitize_terminal_text`] conservant les séquences de style SGR
/// (`ESC [ … m`), pour relayer du texte Markdown coloré sans séquence active.
#[must_use]
pub fn sanitize_passthrough_text(text: &str) -> Cow<'_, str> {
    sanitize_with_policy(text, EscapePolicy::KeepSgr)
}

/// Supprime toutes les séquences d'échappement (CSI, OSC, DCS, APC, PM, SOS) d'une chaîne.
#[must_use]
pub fn strip_ansi(text: &str) -> String {
    terminal_segments(text)
        .filter(|segment| matches!(segment.kind, SegmentKind::Char(_)))
        .map(|segment| segment.raw)
        .collect()
}

fn visible_chars(text: &str) -> impl Iterator<Item = char> + '_ {
    terminal_segments(text).filter_map(|segment| match segment.kind {
        SegmentKind::Char(ch) => Some(ch),
        SegmentKind::Sgr | SegmentKind::OtherEscape => None,
    })
}

/// Calcule la largeur d'affichage réelle en colonnes terminales en ignorant les séquences ANSI
/// et en comptabilisant les caractères double-chasse CJK et emojis (2 colonnes).
#[must_use]
pub fn display_width(text: &str) -> usize {
    visible_chars(text)
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .fold(0_usize, usize::saturating_add)
}

/// Variante calculant la largeur selon la norme CJK East Asian Ambiguous.
#[must_use]
pub fn cjk_display_width(text: &str) -> usize {
    visible_chars(text)
        .map(|ch| UnicodeWidthChar::width_cjk(ch).unwrap_or(0))
        .fold(0_usize, usize::saturating_add)
}

fn copy_ansi_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>, result: &mut String) {
    if chars.peek() == Some(&'[') {
        chars.next();
        result.push('[');
        while let Some(&param_ch) = chars.peek() {
            chars.next();
            result.push(param_ch);
            if ('@'..='~').contains(&param_ch) {
                break;
            }
        }
    }
}

/// Tronque une chaîne pour qu'elle n'excède pas `max_cols` colonnes visuelles
/// sans couper un caractère CJK ou emoji en deux.
#[must_use]
pub fn fit_to_width(text: &str, max_cols: usize) -> String {
    let mut result = String::with_capacity(text.len());
    let mut current_width = 0_usize;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            result.push(ch);
            copy_ansi_sequence(&mut chars, &mut result);
            continue;
        }
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if current_width.saturating_add(char_width) > max_cols {
            break;
        }
        result.push(ch);
        current_width = current_width.saturating_add(char_width);
    }
    result
}

/// Complète une chaîne avec des espaces pour atteindre exactement `target_cols` en largeur visuelle.
#[must_use]
pub fn pad_to_width(text: &str, target_cols: usize) -> String {
    let width = display_width(text);
    if width >= target_cols {
        text.to_string()
    } else {
        let padding = " ".repeat(target_cols.saturating_sub(width));
        format!("{text}{padding}")
    }
}

/// Erreurs métier du domaine de rendu et parsing.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    #[error("Erreur syntaxe Mermaid: {0}")]
    MermaidSyntax(String),
    #[error("Erreur parsing SVG: {0}")]
    SvgParsing(String),
    #[error("Erreur allocation mémoire pixmap: {0}")]
    PixmapAllocation(String),
    #[error("Erreur encodage image: {0}")]
    ImageEncoding(String),
    #[error("Erreur I/O: {0}")]
    Io(String),
    #[error("Limite de ressources dépassée: {0}")]
    ResourceLimit(String),
    #[error("Erreur initialisation terminal: {0}")]
    TerminalInit(String),
    #[error("Erreur ligne de commande: {0}")]
    CommandLine(String),
    #[error("Polices indisponibles: {0}")]
    FontUnavailable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_width_ascii() {
        assert_eq!(display_width("Hello world!"), 12);
        assert_eq!(display_width(""), 0);
    }

    #[test]
    fn test_display_width_cjk_fullwidth() {
        assert_eq!(display_width("你好世界"), 8);
        assert_eq!(display_width("한글"), 4);
        assert_eq!(display_width("日本語"), 6);
    }

    #[test]
    fn test_display_width_emojis() {
        assert_eq!(display_width("🦀"), 2);
        assert_eq!(display_width("🚀"), 2);
        assert_eq!(display_width("Rust 🦀 2026"), 12);
    }

    #[test]
    fn test_display_width_ignores_ansi_styling() {
        let plain = "Rust 🦀 编程";
        let colored = "\x1b[32m\x1b[1mRust 🦀 编程\x1b[0m";
        assert_eq!(display_width(plain), 12);
        assert_eq!(display_width(colored), 12);
    }

    #[test]
    fn test_fit_to_width_boundary() {
        let text = "你好世界";
        assert_eq!(fit_to_width(text, 8), "你好世界");
        assert_eq!(fit_to_width(text, 5), "你好");
        assert_eq!(fit_to_width(text, 4), "你好");
        assert_eq!(fit_to_width(text, 3), "你");
        assert_eq!(fit_to_width(text, 1), "");
    }

    #[test]
    fn test_pad_to_width() {
        assert_eq!(pad_to_width("你好", 6), "你好  ");
        assert_eq!(pad_to_width("你好", 4), "你好");
        assert_eq!(pad_to_width("Hello", 8), "Hello   ");
    }

    #[test]
    fn test_viewport_geometry_methods() {
        let vp = ViewportGeometry::new(10, 24);
        assert_eq!(ViewportGeometry::display_width("你好"), 4);
        assert_eq!(vp.fit_line("你好世界ABCDEF"), "你好世界AB");
        assert_eq!(ViewportGeometry::pad_to_width("🦀", 4), "🦀  ");
    }

    #[test]
    fn test_viewport_geometry_target_columns_compaction() {
        assert_eq!(ViewportGeometry::new(160, 24).target_columns(), 144);
        assert_eq!(ViewportGeometry::new(100, 24).target_columns(), 90);
        assert_eq!(ViewportGeometry::new(90, 24).target_columns(), 83);
        assert_eq!(ViewportGeometry::new(80, 24).target_columns(), 74);
        assert_eq!(ViewportGeometry::new(60, 24).target_columns(), 57);
        assert_eq!(ViewportGeometry::new(50, 24).target_columns(), 48);
        assert_eq!(ViewportGeometry::new(40, 24).target_columns(), 40);
        assert_eq!(ViewportGeometry::new(20, 24).target_columns(), 20);
    }

    #[test]
    fn test_viewport_geometry_target_columns_does_not_overflow() {
        assert_eq!(ViewportGeometry::new(8000, 24).target_columns(), 7200);
        assert_eq!(ViewportGeometry::new(u16::MAX, 24).target_columns(), 58981);
    }

    fn opening(line: &str) -> Option<CodeFence> {
        CodeFence::parse_opening(line)
    }

    #[test]
    fn test_code_fence_parses_backtick_and_tilde_openings() {
        let backtick = opening("```mermaid title=\"T\" width=60");
        assert!(backtick.as_ref().is_some_and(CodeFence::is_mermaid));
        let metadata = backtick.map(|fence| fence.metadata()).unwrap_or_default();
        assert_eq!(metadata.title.as_deref(), Some("T"));
        assert_eq!(metadata.width_override, Some(60));

        assert!(opening("~~~mermaid").is_some_and(|f| f.is_mermaid()));
        assert!(opening("````mermaid").is_some_and(|f| f.is_mermaid()));
        assert!(opening("```mermaidjs").is_some_and(|f| f.is_mermaid()));
        assert!(opening("     ```mermaid").is_some_and(|f| f.is_mermaid()));
    }

    #[test]
    fn test_code_fence_rejects_invalid_openings() {
        assert!(opening("``mermaid").is_none());
        assert!(opening("Texte ordinaire").is_none());
        assert!(opening("``` mermaid `inline`").is_none());
        assert!(opening("```mermaid-x").is_some_and(|f| !f.is_mermaid()));
        assert!(opening("```rust").is_some_and(|f| !f.is_mermaid()));
        assert!(opening("~~~ avec `backtick`").is_some());
    }

    #[test]
    fn test_code_fence_closing_rules() {
        let fence = opening("````mermaid").unwrap_or_else(|| unreachable!());
        assert!(fence.is_closed_by("````"));
        assert!(fence.is_closed_by("  `````  "));
        assert!(!fence.is_closed_by("```"));
        assert!(!fence.is_closed_by("````js"));
        assert!(!fence.is_closed_by("~~~~"));
        assert!(!fence.is_closed_by(""));

        let tilde = opening("~~~mermaid").unwrap_or_else(|| unreachable!());
        assert!(tilde.is_closed_by("~~~"));
        assert!(!tilde.is_closed_by("```"));
    }

    #[test]
    fn test_strip_mermaid_fences() {
        let raw1 = "```mermaid\ngraph TD\n  A --> B\n```";
        assert_eq!(strip_mermaid_fences(raw1), "graph TD\n  A --> B");

        let raw2 = "graph TD\n  A --> B";
        assert_eq!(strip_mermaid_fences(raw2), "graph TD\n  A --> B");

        let raw3 = "```\ngraph TD\n  A --> B\n```";
        assert_eq!(strip_mermaid_fences(raw3), "graph TD\n  A --> B");

        let block = DiagramBlock::from_raw(raw1);
        assert_eq!(block.as_str(), "graph TD\n  A --> B");
    }

    #[test]
    fn test_theme_mode_from_str_and_background_rgb() {
        assert_eq!(ThemeMode::from_str_name("dark"), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_str_name("light"), ThemeMode::Light);
        assert_eq!(ThemeMode::from_str_name("neutral"), ThemeMode::Neutral);
        assert_eq!(ThemeMode::from_str_name("amber"), ThemeMode::Amber);
        assert_eq!(ThemeMode::from_str_name("retro-amber"), ThemeMode::Amber);
        assert_eq!(ThemeMode::from_str_name("AMBER"), ThemeMode::Amber);
        assert_eq!(ThemeMode::from_str_name("phosphor"), ThemeMode::Phosphor);
        assert_eq!(
            ThemeMode::from_str_name("retro-phosphor"),
            ThemeMode::Phosphor
        );
        assert_eq!(ThemeMode::from_str_name("neon"), ThemeMode::Neon);
        assert_eq!(ThemeMode::from_str_name("retro-neon"), ThemeMode::Neon);
        assert_eq!(ThemeMode::from_str_name("mono"), ThemeMode::Mono);
        assert_eq!(ThemeMode::from_str_name("retro-mono"), ThemeMode::Mono);
        assert_eq!(ThemeMode::from_str_name("unknown"), ThemeMode::Dark);

        assert_eq!(ThemeMode::Dark.background_rgb(), (30, 30, 30));
        assert_eq!(ThemeMode::Light.background_rgb(), (255, 255, 255));
        assert_eq!(ThemeMode::Neutral.background_rgb(), (255, 255, 255));
        assert_eq!(ThemeMode::Amber.background_rgb(), (18, 16, 8));
        assert_eq!(ThemeMode::Phosphor.background_rgb(), (10, 20, 10));
        assert_eq!(ThemeMode::Neon.background_rgb(), (13, 11, 24));
        assert_eq!(ThemeMode::Mono.background_rgb(), (0, 0, 0));
    }

    #[test]
    fn test_cli_error_display() {
        let err = CliError::ImageEncoding("buffer invalide".to_string());
        assert_eq!(err.to_string(), "Erreur encodage image: buffer invalide");

        let err_io = CliError::Io("fichier introuvable".to_string());
        assert_eq!(err_io.to_string(), "Erreur I/O: fichier introuvable");
    }

    #[test]
    fn test_graphics_protocol_iterm2() {
        assert_eq!(GraphicsProtocol::default(), GraphicsProtocol::Kitty);
        let proto = GraphicsProtocol::Iterm2;
        assert_eq!(proto, GraphicsProtocol::Iterm2);
    }

    #[test]
    fn test_diagram_metadata_parse_fenced_info() {
        let info = r#"title="Architecture Flux" theme=neon width=75"#;
        let meta = DiagramMetadata::parse_fenced_info(info);
        assert_eq!(meta.title.as_deref(), Some("Architecture Flux"));
        assert_eq!(meta.theme_override, Some(ThemeMode::Neon));
        assert_eq!(meta.width_override, Some(75));

        // Test avec injection ANSI et contrôle
        let info_malicious = "title=\"Dangerous\x1b[31m Title\x07\" width=0 theme=invalid";
        let meta_malicious = DiagramMetadata::parse_fenced_info(info_malicious);
        assert_eq!(meta_malicious.title.as_deref(), Some("Dangerous Title"));
        assert_eq!(meta_malicious.width_override, None);
        assert_eq!(meta_malicious.theme_override, Some(ThemeMode::Dark));
    }

    #[test]
    fn test_diagram_block_from_raw_with_metadata() {
        let raw = "```mermaid title=\"Mon Diagramme\" theme=amber\nflowchart TD\n    A --> B\n```";
        let block = DiagramBlock::from_raw(raw);
        assert_eq!(block.as_str(), "flowchart TD\n    A --> B");
        assert_eq!(block.title(), Some("Mon Diagramme"));
        assert_eq!(block.theme_override(), Some(ThemeMode::Amber));
    }

    #[test]
    fn test_strip_ansi() {
        let styled = "\x1b[32mTexte vert\x1b[0m et \x1b[1;34mbleu gras\x1b[0m";
        assert_eq!(strip_ansi(styled), "Texte vert et bleu gras");
        let osc = "\x1b]1337;File=inline=1:base64\x07Texte normal";
        assert_eq!(strip_ansi(osc), "Texte normal");
    }

    #[test]
    fn test_strip_ansi_removes_dcs_and_apc_sequences() {
        assert_eq!(strip_ansi("a\x1bPq#0;2;0;0;0\x1b\\b"), "ab");
        assert_eq!(strip_ansi("a\x1b_Gf=100,a=T;AAAA\x1b\\b"), "ab");
    }

    #[test]
    fn test_sanitize_terminal_text_removes_injection_vectors() {
        for (input, expected) in [
            ("a\x1b]0;PWNED\x07b", "ab"),
            ("a\x1b]0;PWNED\x1b\\b", "ab"),
            ("a\x1b]52;c;ZXZpbA==\x07b", "ab"),
            ("a\x1b[2Jb", "ab"),
            ("a\x1b[31mb\x1b[0m", "ab"),
            ("a\x1bPq#0\x1b\\b", "ab"),
            ("a\x1b_Ga=T;AAAA\x1b\\b", "ab"),
            ("a\x1b^privacy\x1b\\b", "ab"),
            ("a\x1bcb", "ab"),
            ("a\u{9b}2Jb", "a2Jb"),
            ("a\u{9d}0;x\u{9c}b", "a0;xb"),
            ("a\rb\x08c\x7fd", "abcd"),
            ("a\x1b]0;non terminé", "a"),
            ("ligne\n\tindentée", "ligne\n\tindentée"),
        ] {
            assert_eq!(sanitize_terminal_text(input), expected, "entrée {input:?}");
        }
    }

    #[test]
    fn test_sanitize_terminal_text_borrows_clean_text() {
        let clean = "flowchart TD\n  A[café] --> B";
        assert!(matches!(sanitize_terminal_text(clean), Cow::Borrowed(_)));
        assert!(matches!(sanitize_passthrough_text(clean), Cow::Borrowed(_)));
    }

    #[test]
    fn test_sanitize_passthrough_text_keeps_only_sgr() {
        let styled = "\x1b[1;32mvert\x1b[0m \x1b[38:2::255:0:0mrouge\x1b[m";
        assert_eq!(sanitize_passthrough_text(styled), styled);
        assert_eq!(
            sanitize_passthrough_text("\x1b[31ma\x1b]52;c;eA==\x07\x1b[2Jb\x1b[?25lc"),
            "\x1b[31mabc"
        );
    }

    #[test]
    fn test_sanitize_passthrough_text_rejects_malformed_sgr() {
        assert_eq!(sanitize_passthrough_text("a\x1b[31\x07mb"), "amb");
        assert_eq!(sanitize_passthrough_text("a\x1b[>4mb"), "ab");
    }

    #[test]
    fn test_display_width_ignores_apc_payload() {
        assert_eq!(display_width("\x1b_Ga=T;AAAAAAAA\x1b\\ok"), 2);
    }

    #[test]
    fn test_fence_title_strips_osc_sequence() {
        let block =
            DiagramBlock::from_raw("```mermaid title=\"A\x1b]0;x\x07B\"\nflowchart TD\n```");
        assert_eq!(block.title(), Some("AB"));
    }
}
