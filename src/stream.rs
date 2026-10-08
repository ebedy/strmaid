use crate::domain::{CliError, DiagramBlock, DiagramMetadata, ResourceLimits};
use std::io::{self, BufRead, Read};
use std::mem;

/// Ligne d'entrée décodée en UTF-8 de manière tolérante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LossyLine {
    /// Contenu sans le `\n` final ; les séquences invalides deviennent U+FFFD.
    pub text: String,
    /// Indique qu'au moins un octet invalide a été remplacé.
    pub had_invalid_utf8: bool,
}

/// Itérateur de lignes tolérant aux octets non UTF-8, à la différence de
/// `BufRead::lines()` qui interrompt le flux sur la première séquence invalide.
pub struct LossyLines<R> {
    reader: R,
    buffer: Vec<u8>,
    max_line_bytes: Option<usize>,
}

/// Résultat brut de la lecture d'une ligne.
enum RawLine {
    Eof,
    Complete,
    Oversized { bytes: usize, limit: usize },
}

impl<R: BufRead> LossyLines<R> {
    #[must_use]
    pub const fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            max_line_bytes: None,
        }
    }

    /// Variante bornée : une ligne dont le contenu dépasse `max_line_bytes` est drainée
    /// jusqu'au `\n` suivant sans être accumulée, puis signalée par
    /// `CliError::ResourceLimit` ; l'itération se poursuit à la ligne suivante.
    #[must_use]
    pub const fn with_max_line_bytes(reader: R, max_line_bytes: usize) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            max_line_bytes: Some(max_line_bytes),
        }
    }

    fn read_raw_line(&mut self) -> io::Result<RawLine> {
        let Some(max) = self.max_line_bytes else {
            let read = self.reader.read_until(b'\n', &mut self.buffer)?;
            return Ok(if read == 0 {
                RawLine::Eof
            } else {
                RawLine::Complete
            });
        };
        let limit = u64::try_from(max).unwrap_or(u64::MAX).saturating_add(1);
        let read = (&mut self.reader)
            .take(limit)
            .read_until(b'\n', &mut self.buffer)?;
        if read == 0 {
            return Ok(RawLine::Eof);
        }
        if read <= max || self.buffer.last() == Some(&b'\n') {
            return Ok(RawLine::Complete);
        }
        self.buffer.clear();
        let drained = drain_until_newline(&mut self.reader)?;
        Ok(RawLine::Oversized {
            bytes: read.saturating_add(drained),
            limit: max,
        })
    }
}

impl<R: BufRead> Iterator for LossyLines<R> {
    type Item = Result<LossyLine, CliError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_raw_line() {
            Ok(RawLine::Eof) => None,
            Ok(RawLine::Complete) => Some(Ok(decode_lossy_line(mem::take(&mut self.buffer)))),
            Ok(RawLine::Oversized { bytes, limit }) => Some(Err(CliError::ResourceLimit(format!(
                "ligne d'entrée de {bytes} octets supérieure à la limite de {limit} octets"
            )))),
            Err(err) => Some(Err(CliError::Io(err.to_string()))),
        }
    }
}

/// Consomme le flux jusqu'au prochain `\n` inclus sans rien accumuler ; retourne
/// le nombre d'octets écartés.
fn drain_until_newline<R: BufRead>(reader: &mut R) -> io::Result<usize> {
    let mut drained = 0_usize;
    loop {
        let available = match reader.fill_buf() {
            Ok(available) => available,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
            Err(err) => return Err(err),
        };
        if available.is_empty() {
            return Ok(drained);
        }
        let newline = available.iter().position(|&byte| byte == b'\n');
        let consumed = newline.map_or(available.len(), |pos| pos + 1);
        reader.consume(consumed);
        drained = drained.saturating_add(consumed);
        if newline.is_some() {
            return Ok(drained);
        }
    }
}

fn decode_lossy_line(mut bytes: Vec<u8>) -> LossyLine {
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    match String::from_utf8(bytes) {
        Ok(text) => LossyLine {
            text,
            had_invalid_utf8: false,
        },
        Err(err) => LossyLine {
            text: String::from_utf8_lossy(err.as_bytes()).into_owned(),
            had_invalid_utf8: true,
        },
    }
}

/// Émet sur stderr un avertissement unique à la première ligne contenant des octets
/// non UTF-8. Retourne le nouvel état « déjà signalé ».
#[must_use]
pub fn report_invalid_utf8_once(line: &LossyLine, already_reported: bool) -> bool {
    if already_reported || !line.had_invalid_utf8 {
        return already_reported;
    }
    eprintln!("\x1b[33m⚠️  [Octets non UTF-8 remplacés par U+FFFD dans le flux d'entrée]\x1b[0m");
    true
}

/// Élément produit par la machine à états de streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamItem {
    Text(String),
    Diagram(DiagramBlock),
    /// Bloc Mermaid ignoré car son contenu dépasse `ResourceLimits::max_diagram_bytes`.
    /// Le contenu n'est pas conservé ; seul son volume est rapporté.
    OversizedDiagram {
        skipped_bytes: usize,
    },
}

/// Bloc Mermaid en cours d'accumulation.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagramCapture {
    buffer: Vec<String>,
    bytes_used: usize,
    metadata: DiagramMetadata,
    opening_fence: String,
}

impl DiagramCapture {
    fn into_block(self) -> DiagramBlock {
        DiagramBlock::with_metadata(self.buffer.join("\n"), self.metadata)
    }

    fn into_unclosed_text(self) -> String {
        format!("{}\n{}", self.opening_fence, self.buffer.join("\n"))
    }
}

/// État interne de la machine de streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Passthrough,
    CapturingDiagram(DiagramCapture),
    /// Bloc hors quota drainé jusqu'à sa clôture sans accumulation mémoire.
    SkippingOversizedDiagram {
        skipped_bytes: usize,
    },
}

/// Machine à états pour l'analyse d'un flux Markdown en streaming.
#[derive(Debug, Clone)]
pub struct StreamStateMachine {
    state: State,
    limits: ResourceLimits,
}

impl Default for StreamStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamStateMachine {
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(ResourceLimits::default())
    }

    #[must_use]
    pub const fn with_limits(limits: ResourceLimits) -> Self {
        Self {
            state: State::Passthrough,
            limits,
        }
    }

    /// Traite une ligne entrante et retourne un élément si disponible.
    ///
    /// Un bloc dépassant `max_diagram_bytes` n'interrompt jamais le flux : il est
    /// drainé puis signalé par `StreamItem::OversizedDiagram`.
    pub fn process_line(&mut self, line: &str) -> Option<StreamItem> {
        let sanitized = line.strip_suffix('\r').unwrap_or(line);
        match mem::replace(&mut self.state, State::Passthrough) {
            State::Passthrough => self.process_passthrough(sanitized),
            State::CapturingDiagram(capture) => self.process_capture(sanitized, capture),
            State::SkippingOversizedDiagram { skipped_bytes } => {
                self.process_skip(sanitized, skipped_bytes)
            }
        }
    }

    /// Gère une ligne en mode Passthrough.
    fn process_passthrough(&mut self, line: &str) -> Option<StreamItem> {
        let trimmed = line.trim_start();
        let Some(info) = trimmed.strip_prefix("```mermaid") else {
            return Some(StreamItem::Text(line.to_string()));
        };
        self.state = State::CapturingDiagram(DiagramCapture {
            buffer: Vec::new(),
            bytes_used: 0,
            metadata: DiagramMetadata::parse_fenced_info(info),
            opening_fence: line.to_string(),
        });
        None
    }

    /// Gère une ligne en mode Accumulation de diagramme.
    fn process_capture(&mut self, line: &str, mut capture: DiagramCapture) -> Option<StreamItem> {
        if is_closing_fence(line) {
            return Some(StreamItem::Diagram(capture.into_block()));
        }
        let next_bytes = accumulate_line_bytes(capture.bytes_used, line);
        if next_bytes > self.limits.max_diagram_bytes {
            self.state = State::SkippingOversizedDiagram {
                skipped_bytes: next_bytes,
            };
            return None;
        }
        capture.buffer.push(line.to_string());
        capture.bytes_used = next_bytes;
        self.state = State::CapturingDiagram(capture);
        None
    }

    /// Gère une ligne d'un bloc hors quota : seul le volume est comptabilisé.
    fn process_skip(&mut self, line: &str, skipped_bytes: usize) -> Option<StreamItem> {
        if is_closing_fence(line) {
            return Some(StreamItem::OversizedDiagram { skipped_bytes });
        }
        self.state = State::SkippingOversizedDiagram {
            skipped_bytes: accumulate_line_bytes(skipped_bytes, line),
        };
        None
    }

    /// Finalise le flux à la réception d'EOF.
    pub fn finish(&mut self) -> Option<StreamItem> {
        match mem::replace(&mut self.state, State::Passthrough) {
            State::Passthrough => None,
            State::CapturingDiagram(capture) => {
                Some(StreamItem::Text(capture.into_unclosed_text()))
            }
            State::SkippingOversizedDiagram { skipped_bytes } => {
                Some(StreamItem::OversizedDiagram { skipped_bytes })
            }
        }
    }
}

fn is_closing_fence(line: &str) -> bool {
    line.trim().starts_with("```")
}

/// Volume cumulé d'un bloc après ajout d'une ligne et de son saut de ligne.
const fn accumulate_line_bytes(bytes_used: usize, line: &str) -> usize {
    bytes_used.saturating_add(line.len()).saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_pure_text() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Hello world"),
            Some(StreamItem::Text("Hello world".to_string()))
        );
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_mermaid_block() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Before diagram"),
            Some(StreamItem::Text("Before diagram".to_string()))
        );
        assert_eq!(machine.process_line("```mermaid"), None);
        assert_eq!(machine.process_line("graph TD"), None);
        assert_eq!(machine.process_line("  A --> B"), None);
        let finished = machine.process_line("```");
        assert_eq!(
            finished,
            Some(StreamItem::Diagram(DiagramBlock::new(
                "graph TD\n  A --> B".to_string()
            )))
        );
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_unclosed_block_at_eof() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(machine.process_line("```mermaid"), None);
        assert_eq!(machine.process_line("graph LR"), None);
        let eof = machine.finish();
        assert_eq!(
            eof,
            Some(StreamItem::Text("```mermaid\ngraph LR".to_string()))
        );
    }

    fn small_limits_machine() -> StreamStateMachine {
        StreamStateMachine::with_limits(ResourceLimits {
            max_diagram_bytes: 4,
            ..ResourceLimits::default()
        })
    }

    #[test]
    fn test_stream_skips_oversized_block_and_resumes() {
        let mut machine = small_limits_machine();

        assert_eq!(machine.process_line("```mermaid"), None);
        assert_eq!(machine.process_line("graph LR"), None);
        assert_eq!(machine.process_line("more"), None);
        assert_eq!(
            machine.process_line("```"),
            Some(StreamItem::OversizedDiagram { skipped_bytes: 14 })
        );
        assert_eq!(
            machine.process_line("apres"),
            Some(StreamItem::Text("apres".to_string()))
        );
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_oversized_block_at_eof_reports_skip() {
        let mut machine = small_limits_machine();
        assert_eq!(machine.process_line("```mermaid"), None);
        assert_eq!(machine.process_line("graph LR"), None);
        assert_eq!(
            machine.finish(),
            Some(StreamItem::OversizedDiagram { skipped_bytes: 9 })
        );
    }

    #[test]
    fn test_stream_block_at_exact_limit_is_kept() {
        let mut machine = StreamStateMachine::with_limits(ResourceLimits {
            max_diagram_bytes: 4,
            ..ResourceLimits::default()
        });
        assert_eq!(machine.process_line("```mermaid"), None);
        assert_eq!(machine.process_line("abc"), None);
        assert!(matches!(
            machine.process_line("```"),
            Some(StreamItem::Diagram(_))
        ));
    }

    #[test]
    fn test_stream_crlf_normalization() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Ligne avec CRLF\r"),
            Some(StreamItem::Text("Ligne avec CRLF".to_string()))
        );
        assert_eq!(machine.process_line("```mermaid\r"), None);
        assert_eq!(machine.process_line("graph LR\r"), None);
        let finished = machine.process_line("```\r");
        assert_eq!(
            finished,
            Some(StreamItem::Diagram(DiagramBlock::new(
                "graph LR".to_string()
            )))
        );
    }

    #[test]
    fn test_stream_mermaid_block_with_metadata() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line(r#"```mermaid title="Pipeline Flux" theme=neon width=70"#),
            None
        );
        assert_eq!(machine.process_line("flowchart TD"), None);
        let finished = machine.process_line("```");
        assert!(matches!(finished, Some(StreamItem::Diagram(_))));
        if let Some(StreamItem::Diagram(block)) = finished {
            assert_eq!(block.as_str(), "flowchart TD");
            assert_eq!(block.title(), Some("Pipeline Flux"));
            assert_eq!(block.theme_override(), Some(crate::domain::ThemeMode::Neon));
            assert_eq!(block.width_override(), Some(70));
        }
    }

    fn collect_lossy(input: &[u8]) -> Vec<(String, bool)> {
        LossyLines::new(input)
            .map(|line| line.map(|l| (l.text, l.had_invalid_utf8)))
            .collect::<Result<_, _>>()
            .unwrap_or_default()
    }

    #[test]
    fn test_lossy_lines_replaces_invalid_bytes_and_continues() {
        assert_eq!(
            collect_lossy(b"ok\n\xff\xfe\nsuite\n"),
            vec![
                ("ok".to_string(), false),
                ("\u{fffd}\u{fffd}".to_string(), true),
                ("suite".to_string(), false),
            ]
        );
    }

    fn collect_bounded(input: &[u8], max: usize) -> Vec<Result<String, CliError>> {
        LossyLines::with_max_line_bytes(input, max)
            .map(|line| line.map(|l| l.text))
            .collect()
    }

    #[test]
    fn test_lossy_lines_bounded_rejects_long_line_and_resumes() {
        let input = format!("court\n{}\nsuivante\n", "x".repeat(20));
        let lines = collect_bounded(input.as_bytes(), 10);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], Ok("court".to_string()));
        assert!(
            matches!(&lines[1], Err(CliError::ResourceLimit(msg)) if msg.contains("21 octets"))
        );
        assert_eq!(lines[2], Ok("suivante".to_string()));
    }

    #[test]
    fn test_lossy_lines_bounded_accepts_line_at_exact_limit() {
        let input = format!("{}\n{}", "a".repeat(10), "b".repeat(10));
        assert_eq!(
            collect_bounded(input.as_bytes(), 10),
            vec![Ok("a".repeat(10)), Ok("b".repeat(10))]
        );
    }

    #[test]
    fn test_lossy_lines_bounded_rejects_long_last_line_without_newline() {
        let lines = collect_bounded("y".repeat(11).as_bytes(), 10);
        assert_eq!(lines.len(), 1);
        assert!(matches!(&lines[0], Err(CliError::ResourceLimit(_))));
    }

    #[test]
    fn test_lossy_lines_keeps_last_line_without_newline() {
        assert_eq!(
            collect_lossy(b"a\nfin"),
            vec![("a".to_string(), false), ("fin".to_string(), false)]
        );
    }

    #[test]
    fn test_lossy_lines_handles_empty_input_and_empty_lines() {
        assert_eq!(collect_lossy(b""), Vec::new());
        assert_eq!(
            collect_lossy(b"\n\n"),
            vec![(String::new(), false), (String::new(), false)]
        );
    }

    #[test]
    fn test_lossy_lines_keeps_carriage_return_for_state_machine() {
        assert_eq!(
            collect_lossy(b"crlf\r\n"),
            vec![("crlf\r".to_string(), false)]
        );
    }

    #[test]
    fn test_stream_unclosed_block_preserves_opening_fence() {
        let mut machine = StreamStateMachine::new();
        let fence = "```mermaid title=\"Incomplet\"";
        assert_eq!(machine.process_line(fence), None);
        assert_eq!(machine.process_line("graph TD"), None);
        let unclosed = machine.finish();
        assert_eq!(
            unclosed,
            Some(StreamItem::Text(format!("{fence}\ngraph TD")))
        );
    }
}
