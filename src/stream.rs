use crate::domain::{CliError, DiagramBlock, DiagramMetadata, ResourceLimits};
use std::mem;

/// Élément produit par la machine à états de streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamItem {
    Text(String),
    Diagram(DiagramBlock),
}

/// État interne de la machine de streaming.
#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Passthrough,
    CapturingDiagram {
        buffer: Vec<String>,
        bytes_used: usize,
        metadata: DiagramMetadata,
        opening_fence: String,
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
    /// # Errors
    /// Renvoie `CliError::ResourceLimit` si un bloc Mermaid dépasse la limite configurée.
    pub fn process_line(&mut self, line: &str) -> Result<Option<StreamItem>, CliError> {
        let sanitized = line.strip_suffix('\r').unwrap_or(line);
        match mem::replace(&mut self.state, State::Passthrough) {
            State::Passthrough => Ok(self.process_passthrough(sanitized)),
            State::CapturingDiagram {
                buffer,
                bytes_used,
                metadata,
                opening_fence,
            } => self.process_capture(sanitized, buffer, bytes_used, metadata, opening_fence),
        }
    }

    /// Gère une ligne en mode Passthrough.
    fn process_passthrough(&mut self, line: &str) -> Option<StreamItem> {
        let trimmed = line.trim_start();
        if let Some(info) = trimmed.strip_prefix("```mermaid") {
            let metadata = DiagramMetadata::parse_fenced_info(info);
            self.state = State::CapturingDiagram {
                buffer: Vec::new(),
                bytes_used: 0,
                metadata,
                opening_fence: line.to_string(),
            };
            None
        } else {
            self.state = State::Passthrough;
            Some(StreamItem::Text(line.to_string()))
        }
    }

    /// Gère une ligne en mode Accumulation de diagramme.
    fn process_capture(
        &mut self,
        line: &str,
        mut buffer: Vec<String>,
        bytes_used: usize,
        metadata: DiagramMetadata,
        opening_fence: String,
    ) -> Result<Option<StreamItem>, CliError> {
        if line.trim().starts_with("```") {
            let content = buffer.join("\n");
            self.state = State::Passthrough;
            Ok(Some(StreamItem::Diagram(DiagramBlock::with_metadata(
                content, metadata,
            ))))
        } else {
            let next_bytes = bytes_used.saturating_add(line.len()).saturating_add(1);
            if next_bytes > self.limits.max_diagram_bytes {
                self.state = State::Passthrough;
                return Err(CliError::ResourceLimit(format!(
                    "bloc Mermaid supérieur à {} octets",
                    self.limits.max_diagram_bytes
                )));
            }
            buffer.push(line.to_string());
            self.state = State::CapturingDiagram {
                buffer,
                bytes_used: next_bytes,
                metadata,
                opening_fence,
            };
            Ok(None)
        }
    }

    /// Finalise le flux à la réception d'EOF.
    pub fn finish(&mut self) -> Option<StreamItem> {
        if let State::CapturingDiagram {
            buffer,
            opening_fence,
            ..
        } = mem::replace(&mut self.state, State::Passthrough)
        {
            let unclosed = format!("{opening_fence}\n{}", buffer.join("\n"));
            Some(StreamItem::Text(unclosed))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_pure_text() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Hello world"),
            Ok(Some(StreamItem::Text("Hello world".to_string())))
        );
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_mermaid_block() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Before diagram"),
            Ok(Some(StreamItem::Text("Before diagram".to_string())))
        );
        assert_eq!(machine.process_line("```mermaid"), Ok(None));
        assert_eq!(machine.process_line("graph TD"), Ok(None));
        assert_eq!(machine.process_line("  A --> B"), Ok(None));
        let finished = machine.process_line("```");
        assert_eq!(
            finished,
            Ok(Some(StreamItem::Diagram(DiagramBlock::new(
                "graph TD\n  A --> B".to_string()
            ))))
        );
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_unclosed_block_at_eof() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(machine.process_line("```mermaid"), Ok(None));
        assert_eq!(machine.process_line("graph LR"), Ok(None));
        let eof = machine.finish();
        assert_eq!(
            eof,
            Some(StreamItem::Text("```mermaid\ngraph LR".to_string()))
        );
    }

    #[test]
    fn test_stream_mermaid_block_rejects_oversized_content() {
        let limits = ResourceLimits {
            max_diagram_bytes: 4,
            ..ResourceLimits::default()
        };
        let mut machine = StreamStateMachine::with_limits(limits);

        assert_eq!(machine.process_line("```mermaid"), Ok(None));
        let result = machine.process_line("graph LR");

        assert!(matches!(result, Err(CliError::ResourceLimit(_))));
        assert_eq!(machine.finish(), None);
    }

    #[test]
    fn test_stream_crlf_normalization() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line("Ligne avec CRLF\r"),
            Ok(Some(StreamItem::Text("Ligne avec CRLF".to_string())))
        );
        assert_eq!(machine.process_line("```mermaid\r"), Ok(None));
        assert_eq!(machine.process_line("graph LR\r"), Ok(None));
        let finished = machine.process_line("```\r");
        assert_eq!(
            finished,
            Ok(Some(StreamItem::Diagram(DiagramBlock::new(
                "graph LR".to_string()
            ))))
        );
    }

    #[test]
    fn test_stream_mermaid_block_with_metadata() {
        let mut machine = StreamStateMachine::new();
        assert_eq!(
            machine.process_line(r#"```mermaid title="Pipeline Flux" theme=neon width=70"#),
            Ok(None)
        );
        assert_eq!(machine.process_line("flowchart TD"), Ok(None));
        let finished = machine.process_line("```");
        assert!(matches!(finished, Ok(Some(StreamItem::Diagram(_)))));
        if let Ok(Some(StreamItem::Diagram(block))) = finished {
            assert_eq!(block.as_str(), "flowchart TD");
            assert_eq!(block.title(), Some("Pipeline Flux"));
            assert_eq!(block.theme_override(), Some(crate::domain::ThemeMode::Neon));
            assert_eq!(block.width_override(), Some(70));
        }
    }

    #[test]
    fn test_stream_unclosed_block_preserves_opening_fence() {
        let mut machine = StreamStateMachine::new();
        let fence = "```mermaid title=\"Incomplet\"";
        assert_eq!(machine.process_line(fence), Ok(None));
        assert_eq!(machine.process_line("graph TD"), Ok(None));
        let unclosed = machine.finish();
        assert_eq!(
            unclosed,
            Some(StreamItem::Text(format!("{fence}\ngraph TD")))
        );
    }
}
