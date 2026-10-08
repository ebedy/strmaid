use crate::domain::{CliError, CodeFence, DiagramBlock, strip_ansi};
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

/// Taille maximale d'une ligne partielle retenue en attente d'un éventuel fence ;
/// au-delà, elle est relayée telle quelle pour borner la mémoire.
const MAX_PENDING_LINE_BYTES: usize = 64 * 1024;

/// Nombre de fragments PTY (2 Kio chacun) en transit entre le thread lecteur et le
/// processeur ; au-delà, le lecteur est bloqué, ce qui propage la contre-pression
/// à l'enfant au lieu d'accumuler sa sortie en mémoire.
const PTY_CHANNEL_CAPACITY: usize = 64;

/// État de l'intercepteur de flux PTY.
#[derive(Debug, PartialEq, Eq)]
enum InterceptorState {
    Passthrough,
    /// Bloc de code non Mermaid relayé tel quel jusqu'à sa clôture.
    InForeignFence(CodeFence),
    Capturing(CaptureProgress),
}

/// Bloc Mermaid en cours de capture.
#[derive(Debug, PartialEq, Eq)]
struct CaptureProgress {
    buffer: Vec<String>,
    fence: CodeFence,
    opening_fence: String,
    bytes_used: usize,
}

/// Écrit du texte dans le terminal brut en garantissant la séquence CR-LF.
///
/// Chaque `\n` nu devient `\r\n` ; les `\r\n` existants et les `\r` isolés
/// (barres de progression, réécriture de ligne) sont conservés tels quels.
///
/// # Errors
/// Renvoie `CliError::Io` en cas d'erreur d'écriture.
pub fn write_raw_crlf<W: Write>(writer: &mut W, text: &str) -> Result<(), CliError> {
    let to_io_error = |e: std::io::Error| CliError::Io(e.to_string());
    for segment in text.split_inclusive('\n') {
        let Some(body) = segment.strip_suffix('\n') else {
            writer.write_all(segment.as_bytes()).map_err(to_io_error)?;
            continue;
        };
        let line_ending: &[u8] = if body.ends_with('\r') { b"\n" } else { b"\r\n" };
        writer.write_all(body.as_bytes()).map_err(to_io_error)?;
        writer.write_all(line_ending).map_err(to_io_error)?;
    }
    writer.flush().map_err(to_io_error)
}

/// Décodeur UTF-8 incrémental tolérant aux séquences coupées entre deux chunks.
///
/// Une séquence incomplète en fin de chunk (au plus 3 octets) est retenue jusqu'au
/// chunk suivant ; seules les séquences réellement invalides deviennent U+FFFD.
#[derive(Debug, Default)]
pub struct Utf8ChunkDecoder {
    pending: Vec<u8>,
}

impl Utf8ChunkDecoder {
    /// Décode un chunk en retenant l'éventuelle séquence incomplète finale.
    pub fn decode(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        let tail = self.pending.split_off(incomplete_tail_start(&self.pending));
        let decoded = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending = tail;
        decoded
    }

    /// Restitue le reliquat en fin de flux, remplacé par U+FFFD s'il est tronqué.
    pub fn finish(&mut self) -> String {
        let remainder = std::mem::take(&mut self.pending);
        String::from_utf8_lossy(&remainder).into_owned()
    }
}

/// Position du début d'une séquence UTF-8 valide mais incomplète en fin de tampon,
/// ou longueur du tampon si la fin est complète ou invalide.
fn incomplete_tail_start(bytes: &[u8]) -> usize {
    let search_from = bytes.len().saturating_sub(3);
    bytes[search_from..]
        .iter()
        .rposition(|byte| !is_utf8_continuation(*byte))
        .map(|offset| search_from + offset)
        .filter(|&start| {
            matches!(std::str::from_utf8(&bytes[start..]), Err(e) if e.error_len().is_none())
        })
        .unwrap_or(bytes.len())
}

const fn is_utf8_continuation(byte: u8) -> bool {
    byte & 0b1100_0000 == 0b1000_0000
}

/// Processeur de flux interceptant les blocs Mermaid dans la sortie du PTY.
pub struct PtyStreamProcessor<W: Write> {
    writer: W,
    options: RenderOptions,
    state: InterceptorState,
    pending_line: String,
    decoder: Utf8ChunkDecoder,
}

impl<W: Write> PtyStreamProcessor<W> {
    #[must_use]
    pub fn new(writer: W, options: RenderOptions) -> Self {
        Self {
            writer,
            options,
            state: InterceptorState::Passthrough,
            pending_line: String::new(),
            decoder: Utf8ChunkDecoder::default(),
        }
    }

    /// Traite un fragment d'octets bruts issu du PTY, décodé en UTF-8 incrémental.
    ///
    /// # Errors
    /// Renvoie `CliError::Io` en cas d'erreur d'écriture.
    pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<(), CliError> {
        let text = self.decoder.decode(bytes);
        self.process_chunk(&text)
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

        self.abort_capture_if_pending_overflows()?;
        self.flush_pending_interactive()
    }

    fn process_completed_line(&mut self, line: &str) -> Result<(), CliError> {
        match std::mem::replace(&mut self.state, InterceptorState::Passthrough) {
            InterceptorState::Passthrough => self.handle_passthrough_line(line),
            InterceptorState::InForeignFence(fence) => self.handle_foreign_line(line, fence),
            InterceptorState::Capturing(capture) => self.handle_capturing_line(line, capture),
        }
    }

    fn handle_passthrough_line(&mut self, line: &str) -> Result<(), CliError> {
        let Some(fence) = CodeFence::parse_opening(&strip_ansi(line)) else {
            return self.write_line(line);
        };
        if !fence.is_mermaid() {
            self.state = InterceptorState::InForeignFence(fence);
            return self.write_line(line);
        }
        self.state = InterceptorState::Capturing(CaptureProgress {
            buffer: Vec::new(),
            fence,
            opening_fence: line.to_string(),
            bytes_used: 0,
        });
        Ok(())
    }

    fn handle_foreign_line(&mut self, line: &str, fence: CodeFence) -> Result<(), CliError> {
        if !fence.is_closed_by(&strip_ansi(line)) {
            self.state = InterceptorState::InForeignFence(fence);
        }
        self.write_line(line)
    }

    fn write_line(&mut self, line: &str) -> Result<(), CliError> {
        write_raw_crlf(&mut self.writer, line)?;
        write_raw_crlf(&mut self.writer, "\n")
    }

    fn handle_capturing_line(
        &mut self,
        line: &str,
        mut capture: CaptureProgress,
    ) -> Result<(), CliError> {
        if capture.fence.is_closed_by(&strip_ansi(line)) {
            let diagram =
                DiagramBlock::with_metadata(capture.buffer.join("\n"), capture.fence.metadata());
            let rendered = renderer::render_diagram(&diagram, self.options);
            return write_raw_crlf(&mut self.writer, &rendered);
        }
        let next_bytes = capture
            .bytes_used
            .saturating_add(line.len())
            .saturating_add(1);
        if next_bytes > self.options.limits.max_diagram_bytes {
            self.abandon_capture(capture)?;
            return self.write_line(line);
        }
        capture.buffer.push(strip_ansi(line));
        capture.bytes_used = next_bytes;
        self.state = InterceptorState::Capturing(capture);
        Ok(())
    }

    /// Restitue tel quel un bloc dont la capture est abandonnée (dépassement de
    /// `max_diagram_bytes` ou fin de flux) ; la suite du bloc, clôture comprise, est
    /// ensuite relayée comme un bloc de code non Mermaid.
    fn abandon_capture(&mut self, capture: CaptureProgress) -> Result<(), CliError> {
        self.write_line(&capture.opening_fence)?;
        capture
            .buffer
            .iter()
            .try_for_each(|line| self.write_line(line))?;
        self.state = InterceptorState::InForeignFence(capture.fence);
        Ok(())
    }

    /// Abandonne l'éventuelle capture en cours en restituant son contenu.
    fn abandon_pending_capture(&mut self) -> Result<(), CliError> {
        match std::mem::replace(&mut self.state, InterceptorState::Passthrough) {
            InterceptorState::Capturing(capture) => self.abandon_capture(capture),
            InterceptorState::InForeignFence(fence) => {
                self.state = InterceptorState::InForeignFence(fence);
                Ok(())
            }
            InterceptorState::Passthrough => Ok(()),
        }
    }

    /// Abandonne la capture si la ligne partielle en cours ferait dépasser le quota.
    fn abort_capture_if_pending_overflows(&mut self) -> Result<(), CliError> {
        let InterceptorState::Capturing(capture) = &self.state else {
            return Ok(());
        };
        let projected = capture.bytes_used.saturating_add(self.pending_line.len());
        if projected <= self.options.limits.max_diagram_bytes {
            return Ok(());
        }
        self.abandon_pending_capture()
    }

    fn flush_pending_interactive(&mut self) -> Result<(), CliError> {
        if matches!(self.state, InterceptorState::Capturing(_)) {
            return Ok(());
        }
        let clean = strip_ansi(&self.pending_line);
        let may_open_fence = clean.trim_start().starts_with(['`', '~']);
        if may_open_fence && self.pending_line.len() <= MAX_PENDING_LINE_BYTES {
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
        let undecoded_tail = self.decoder.finish();
        if !undecoded_tail.is_empty() {
            self.process_chunk(&undecoded_tail)?;
        }
        self.abandon_pending_capture()?;
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

    let _stdin_thread = if io::stdin().is_terminal() {
        Some(thread::spawn(move || {
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
        }))
    } else {
        drop(master_writer);
        None
    };

    let (tx, rx) = std::sync::mpsc::sync_channel(PTY_CHANNEL_CAPACITY);
    let reader_thread = thread::spawn(move || {
        let mut read_buf = [0u8; 2048];
        loop {
            match master_reader.read(&mut read_buf) {
                Ok(0) => break,
                Ok(n) => {
                    if tx.send(read_buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
    });

    let stdout = io::stdout();
    let mut processor = PtyStreamProcessor::new(stdout.lock(), options);

    loop {
        match rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(bytes) => processor.process_bytes(&bytes)?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(_)) = child.try_wait() {
                    while let Ok(bytes) = rx.try_recv() {
                        processor.process_bytes(&bytes)?;
                    }
                    break;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    processor.finish()?;

    drop(rx);
    drop(pair.master);
    let _ = reader_thread.join();

    let exit_status = child
        .wait()
        .map_err(|e| CliError::Io(format!("Échec attente processus PTY: {e}")))?;

    Ok(exit_status.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ResourceLimits;

    fn processed_output(chunks: &[&str]) -> String {
        let mut out = Vec::new();
        let options = RenderOptions::new(
            crate::domain::ThemeMode::Mono,
            crate::domain::GraphicsProtocol::AsciiBox,
            crate::domain::ViewportGeometry::new(80, 24),
        );
        let mut processor = PtyStreamProcessor::new(&mut out, options);
        assert!(
            chunks
                .iter()
                .all(|chunk| processor.process_chunk(chunk).is_ok())
        );
        assert!(processor.finish().is_ok());
        String::from_utf8(out).unwrap_or_default()
    }

    #[test]
    fn test_pty_processor_detects_ansi_colored_fences() {
        let output = processed_output(&[
            "\x1b[32m```mermaid\x1b[0m\ngraph TD\n  A --> B\n\x1b[0m```\x1b[0m\nfin\n",
        ]);
        assert!(!output.contains("```mermaid"), "sortie : {output:?}");
        assert!(output.contains("fin"));
    }

    #[test]
    fn test_pty_processor_holds_tilde_fence_split_across_chunks() {
        let output = processed_output(&["~~", "~mermaid\ngraph TD\n  A --> B\n~~~\n"]);
        assert!(!output.contains("~~~mermaid"), "sortie : {output:?}");
        assert!(!output.contains("A --> B"), "sortie : {output:?}");
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
    fn test_write_raw_crlf_preserves_lone_carriage_returns() {
        for (input, expected) in [
            ("a\rb\n", "a\rb\r\n"),
            ("a\r\n", "a\r\n"),
            ("\n", "\r\n"),
            ("10%\r20%\r", "10%\r20%\r"),
        ] {
            let mut out = Vec::new();
            assert!(write_raw_crlf(&mut out, input).is_ok());
            assert_eq!(String::from_utf8(out).unwrap_or_default(), expected);
        }
    }

    fn decode_chunks(chunks: &[&[u8]]) -> String {
        let mut decoder = Utf8ChunkDecoder::default();
        let text: String = chunks.iter().map(|chunk| decoder.decode(chunk)).collect();
        text + &decoder.finish()
    }

    #[test]
    fn test_utf8_decoder_joins_two_byte_char_split_across_chunks() {
        assert_eq!(decode_chunks(&[b"caf\xc3", b"\xa9\n"]), "café\n");
    }

    #[test]
    fn test_utf8_decoder_joins_four_byte_char_split_one_plus_three() {
        assert_eq!(decode_chunks(&[b"\xf0", b"\x9f\x98\x80"]), "😀");
    }

    #[test]
    fn test_utf8_decoder_replaces_isolated_invalid_byte_once() {
        assert_eq!(decode_chunks(&[b"a\xffb"]), "a\u{fffd}b");
    }

    #[test]
    fn test_utf8_decoder_replaces_truncated_tail_on_finish() {
        let mut decoder = Utf8ChunkDecoder::default();
        assert_eq!(decoder.decode(b"ok\xc3"), "ok");
        assert_eq!(decoder.finish(), "\u{fffd}");
        assert_eq!(decoder.finish(), "");
    }

    #[test]
    fn test_utf8_decoder_handles_empty_input() {
        assert_eq!(decode_chunks(&[b"", b""]), "");
    }

    #[test]
    fn test_pty_processor_decodes_bytes_split_across_chunks() {
        let mut out = Vec::new();
        let mut processor = PtyStreamProcessor::new(&mut out, RenderOptions::default());
        assert!(processor.process_bytes(b"\xc3").is_ok());
        assert!(processor.process_bytes(b"\xa9t\xc3").is_ok());
        assert!(processor.finish().is_ok());
        assert_eq!(String::from_utf8(out).unwrap_or_default(), "ét\u{fffd}");
    }

    fn small_capture_options() -> RenderOptions {
        let defaults = RenderOptions::default();
        RenderOptions::with_limits(
            defaults.theme,
            defaults.protocol,
            defaults.viewport,
            ResourceLimits {
                max_diagram_bytes: 16,
                ..ResourceLimits::default()
            },
        )
    }

    #[test]
    fn test_pty_processor_aborts_oversized_capture_and_reemits_content() {
        let mut out = Vec::new();
        let mut processor = PtyStreamProcessor::new(&mut out, small_capture_options());
        let input = "```mermaid\naaaaaaaaaa\nbbbbbbbbbb\n```\napres\n";
        assert!(processor.process_chunk(input).is_ok());
        assert_eq!(processor.state, InterceptorState::Passthrough);
        assert!(processor.finish().is_ok());

        let out_str = String::from_utf8(out).unwrap_or_default();
        assert_eq!(
            out_str,
            "```mermaid\r\naaaaaaaaaa\r\nbbbbbbbbbb\r\n```\r\napres\r\n"
        );
    }

    #[test]
    fn test_pty_processor_aborts_capture_when_pending_line_overflows() {
        let mut out = Vec::new();
        let mut processor = PtyStreamProcessor::new(&mut out, small_capture_options());
        let long_line = "c".repeat(40);
        assert!(processor.process_chunk("```mermaid\n").is_ok());
        assert!(processor.process_chunk(&long_line).is_ok());
        assert!(matches!(
            processor.state,
            InterceptorState::InForeignFence(_)
        ));
        assert_eq!(processor.pending_line.len(), 0);

        let out_str = String::from_utf8(out).unwrap_or_default();
        assert_eq!(out_str, format!("```mermaid\r\n{long_line}"));
    }

    #[test]
    fn test_pty_processor_flushes_long_backtick_pending_line() {
        let mut out = Vec::new();
        let mut processor = PtyStreamProcessor::new(&mut out, RenderOptions::default());
        let long_line = format!("`{}", "x".repeat(MAX_PENDING_LINE_BYTES));
        assert!(processor.process_chunk("`court").is_ok());
        assert_eq!(processor.pending_line, "`court");
        assert!(processor.process_chunk(&long_line[1..]).is_ok());
        assert_eq!(processor.pending_line.len(), 0);

        let out_str = String::from_utf8(out).unwrap_or_default();
        assert_eq!(out_str.len(), "`court".len() + long_line.len() - 1);
    }

    #[test]
    fn test_pty_processor_preserves_lone_carriage_return() {
        let mut out = Vec::new();
        let mut processor = PtyStreamProcessor::new(&mut out, RenderOptions::default());
        assert!(processor.process_chunk("a\rb\n").is_ok());
        assert!(processor.finish().is_ok());
        assert_eq!(String::from_utf8(out).unwrap_or_default(), "a\rb\r\n");
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
