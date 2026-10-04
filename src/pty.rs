use crate::domain::{CliError, DiagramBlock, strip_ansi};
use crate::renderer::{self, RenderOptions};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{self, IsTerminal, Read, Write};
use std::thread;

/// Garde RAII pour la gestion du mode brut du terminal hôte.
pub struct RawModeGuard {
    active: bool,
}

impl RawModeGuard {
    /// Active le mode brut si nécessaire et retourne la garde RAII.
    ///
    /// # Errors
    /// Renvoie `CliError::TerminalInit` si l'activation échoue pour une raison autre
    /// que l'absence d'un terminal interactif (ex: headless CI, pipe).
    pub fn enter() -> Result<Self, CliError> {
        if !io::stdin().is_terminal() {
            return Ok(Self { active: false });
        }
        if crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
            return Ok(Self { active: false });
        }
        match crossterm::terminal::enable_raw_mode() {
            Ok(()) => Ok(Self { active: true }),
            Err(e) if is_headless_terminal_error(&e) => Ok(Self { active: false }),
            Err(e) => Err(CliError::TerminalInit(format!(
                "Échec activation raw mode: {e}"
            ))),
        }
    }
}

fn is_headless_terminal_error(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::Unsupported | io::ErrorKind::NotFound | io::ErrorKind::BrokenPipe
    ) || matches!(err.raw_os_error(), Some(6 | 9 | 19 | 25))
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
}

/// État de l'intercepteur de flux PTY.
#[derive(Debug, PartialEq, Eq)]
enum InterceptorState {
    Passthrough,
    Capturing {
        buffer: Vec<String>,
        opening_fence: String,
    },
}

/// Détecte si une ligne correspond au début d'un bloc fenced Mermaid.
#[must_use]
pub fn is_mermaid_fence_start(line: &str) -> bool {
    let clean = strip_ansi(line);
    let trimmed = clean.trim_start();
    trimmed.starts_with("```mermaid")
}

/// Détecte si une ligne correspond à la fermeture d'un bloc fenced.
#[must_use]
pub fn is_fence_end(line: &str) -> bool {
    let clean = strip_ansi(line);
    let trimmed = clean.trim();
    trimmed == "```"
}

/// Écrit du texte dans le terminal brut en garantissant la séquence CR-LF.
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur d'écriture.
pub fn write_raw_crlf<W: Write>(writer: &mut W, text: &str) -> Result<(), CliError> {
    for ch in text.chars() {
        if ch == '\n' {
            writer
                .write_all(b"\r\n")
                .map_err(|e| CliError::Io(e.to_string()))?;
        } else if ch != '\r' {
            let mut buf = [0u8; 4];
            let s = ch.encode_utf8(&mut buf);
            writer
                .write_all(s.as_bytes())
                .map_err(|e| CliError::Io(e.to_string()))?;
        }
    }
    writer.flush().map_err(|e| CliError::Io(e.to_string()))?;
    Ok(())
}

/// Processeur de flux interceptant les blocs Mermaid dans la sortie du PTY.
pub struct PtyStreamProcessor<W: Write> {
    writer: W,
    options: RenderOptions,
    state: InterceptorState,
    pending_line: String,
}

impl<W: Write> PtyStreamProcessor<W> {
    #[must_use]
    pub fn new(writer: W, options: RenderOptions) -> Self {
        Self {
            writer,
            options,
            state: InterceptorState::Passthrough,
            pending_line: String::new(),
        }
    }

    /// Traite un fragment de texte brut issu du PTY.
    ///
    /// # Errors
    /// Renvoie `CliError::Io` en cas d'erreur d'écriture.
    pub fn process_chunk(&mut self, chunk: &str) -> Result<(), CliError> {
        for ch in chunk.chars() {
            if ch == '\n' {
                let line = std::mem::take(&mut self.pending_line);
                let sanitized = line.strip_suffix('\r').unwrap_or(&line);
                self.process_completed_line(sanitized)?;
            } else {
                self.pending_line.push(ch);
            }
        }

        self.flush_pending_interactive()
    }

    fn process_completed_line(&mut self, line: &str) -> Result<(), CliError> {
        match std::mem::replace(&mut self.state, InterceptorState::Passthrough) {
            InterceptorState::Passthrough => self.handle_passthrough_line(line),
            InterceptorState::Capturing {
                buffer,
                opening_fence,
            } => self.handle_capturing_line(line, buffer, opening_fence),
        }
    }

    fn handle_passthrough_line(&mut self, line: &str) -> Result<(), CliError> {
        if is_mermaid_fence_start(line) {
            self.state = InterceptorState::Capturing {
                buffer: Vec::new(),
                opening_fence: line.to_string(),
            };
            Ok(())
        } else {
            self.state = InterceptorState::Passthrough;
            write_raw_crlf(&mut self.writer, line)?;
            write_raw_crlf(&mut self.writer, "\n")
        }
    }

    fn handle_capturing_line(
        &mut self,
        line: &str,
        mut buffer: Vec<String>,
        opening_fence: String,
    ) -> Result<(), CliError> {
        if is_fence_end(line) {
            let raw_content = buffer.join("\n");
            let diagram = DiagramBlock::from_raw(&format!("{opening_fence}\n{raw_content}\n```"));
            let rendered = renderer::render_diagram(&diagram, self.options);
            write_raw_crlf(&mut self.writer, &rendered)?;
            self.state = InterceptorState::Passthrough;
            Ok(())
        } else {
            buffer.push(line.to_string());
            self.state = InterceptorState::Capturing {
                buffer,
                opening_fence,
            };
            Ok(())
        }
    }

    fn flush_pending_interactive(&mut self) -> Result<(), CliError> {
        if self.state != InterceptorState::Passthrough {
            return Ok(());
        }
        let clean = strip_ansi(&self.pending_line);
        let trimmed = clean.trim_start();
        if trimmed.starts_with('`') {
            return Ok(());
        }
        let to_flush = std::mem::take(&mut self.pending_line);
        write_raw_crlf(&mut self.writer, &to_flush)
    }

    /// Finalise le flux à la déconnexion du PTY.
    ///
    /// # Errors
    /// Renvoie `CliError::Io` en cas d'erreur d'écriture.
    pub fn finish(&mut self) -> Result<(), CliError> {
        if let InterceptorState::Capturing {
            buffer,
            opening_fence,
        } = std::mem::replace(&mut self.state, InterceptorState::Passthrough)
        {
            write_raw_crlf(&mut self.writer, &opening_fence)?;
            write_raw_crlf(&mut self.writer, "\n")?;
            for line in buffer {
                write_raw_crlf(&mut self.writer, &line)?;
                write_raw_crlf(&mut self.writer, "\n")?;
            }
        }
        if !self.pending_line.is_empty() {
            let pending = std::mem::take(&mut self.pending_line);
            write_raw_crlf(&mut self.writer, &pending)?;
        }
        Ok(())
    }
}

/// Exécute une commande dans un pseudo-terminal (PTY) interactif.
///
/// # Errors
/// Renvoie `CliError` en cas d'échec d'initialisation, de spawn ou d'I/O.
pub fn run_pty(command: &[String], options: RenderOptions) -> Result<u32, CliError> {
    if command.is_empty() {
        return Err(CliError::CommandLine(
            "Aucune commande spécifiée pour strmaid run".to_string(),
        ));
    }

    let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| CliError::TerminalInit(format!("Échec ouverture PTY: {e}")))?;

    let mut cmd = CommandBuilder::new(&command[0]);
    for arg in &command[1..] {
        cmd.arg(arg);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| CliError::Io(format!("Échec spawn commande: {e}")))?;

    drop(pair.slave);

    let mut master_reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| CliError::Io(format!("Échec clone PTY reader: {e}")))?;

    let mut master_writer = pair
        .master
        .take_writer()
        .map_err(|e| CliError::Io(format!("Échec acquisition PTY writer: {e}")))?;

    let _guard = RawModeGuard::enter()?;

    let _stdin_thread = thread::spawn(move || {
        let mut stdin = io::stdin();
        let mut buf = [0u8; 1024];
        loop {
            match stdin.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if master_writer.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    let _ = master_writer.flush();
                }
            }
        }
    });

    let stdout = io::stdout();
    let mut processor = PtyStreamProcessor::new(stdout.lock(), options);
    let mut read_buf = [0u8; 2048];

    loop {
        match master_reader.read(&mut read_buf) {
            Ok(0) => break,
            Ok(n) => {
                let text = String::from_utf8_lossy(&read_buf[..n]);
                processor.process_chunk(&text)?;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }

    processor.finish()?;

    let exit_status = child
        .wait()
        .map_err(|e| CliError::Io(format!("Échec attente processus PTY: {e}")))?;

    Ok(exit_status.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mermaid_fence_detection() {
        assert!(is_mermaid_fence_start("```mermaid"));
        assert!(is_mermaid_fence_start("  ```mermaid title=\"Test\""));
        assert!(is_mermaid_fence_start("\x1b[32m```mermaid\x1b[0m"));
        assert!(!is_mermaid_fence_start("```rust"));
        assert!(!is_mermaid_fence_start("Texte ordinaire"));

        assert!(is_fence_end("```"));
        assert!(is_fence_end("  ```  "));
        assert!(is_fence_end("\x1b[0m```\x1b[0m"));
        assert!(!is_fence_end("```mermaid"));
    }

    #[test]
    fn test_write_raw_crlf() {
        let mut out = Vec::new();
        let res = write_raw_crlf(&mut out, "Ligne 1\nLigne 2\r\nLigne 3");
        assert!(res.is_ok());
        let res_str = String::from_utf8(out).unwrap_or_default();
        assert_eq!(res_str, "Ligne 1\r\nLigne 2\r\nLigne 3");
    }

    #[test]
    fn test_pty_processor_intercepts_diagram() {
        let mut out = Vec::new();
        let options = RenderOptions::default();
        let mut processor = PtyStreamProcessor::new(&mut out, options);

        let input = "Début du log\n```mermaid\nflowchart TD\n  A --> B\n```\nFin du log\n";
        let res = processor.process_chunk(input);
        assert!(res.is_ok());
        let finish_res = processor.finish();
        assert!(finish_res.is_ok());

        let out_str = String::from_utf8(out).unwrap_or_default();
        assert!(out_str.contains("Début du log"));
        assert!(out_str.contains("Fin du log"));
        assert!(!out_str.contains("```mermaid"));
    }

    #[test]
    fn test_pty_processor_flushes_interactive_prompt() {
        let mut out = Vec::new();
        let options = RenderOptions::default();
        let mut processor = PtyStreamProcessor::new(&mut out, options);

        let prompt = "agent> ";
        let res = processor.process_chunk(prompt);
        assert!(res.is_ok());

        let out_str = String::from_utf8(out).unwrap_or_default();
        assert_eq!(out_str, "agent> ");
    }

    #[test]
    fn test_raw_mode_guard_enter_in_non_interactive_env() {
        let guard = RawModeGuard::enter();
        assert!(guard.is_ok());
    }

    #[test]
    fn test_is_headless_terminal_error_matches_enotty_and_enxio() {
        let enxio = io::Error::from_raw_os_error(6);
        let enotty = io::Error::from_raw_os_error(25);
        let other = io::Error::from_raw_os_error(1);
        assert!(is_headless_terminal_error(&enxio));
        assert!(is_headless_terminal_error(&enotty));
        assert!(!is_headless_terminal_error(&other));
    }
}
