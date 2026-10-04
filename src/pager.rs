use crate::domain::{CliError, ViewportGeometry};
use crate::renderer::{self, RenderOptions};
use crate::stream::{StreamItem, StreamStateMachine};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, BufRead, Stdout};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

/// Gardien RAII pour restaurer l'état du terminal en sortie.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

/// État interne du pager interactif.
#[derive(Debug)]
pub(crate) struct PagerApp {
    lines: Vec<String>,
    scroll: usize,
    auto_scroll: bool,
    stream_finished: bool,
    visible_height: usize,
}

impl PagerApp {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lines: Vec::new(),
            scroll: 0,
            auto_scroll: true,
            stream_finished: false,
            visible_height: 20,
        }
    }

    pub fn add_item(&mut self, item: StreamItem, options: RenderOptions) {
        match item {
            StreamItem::Text(text) => {
                self.lines.push(text);
            }
            StreamItem::Diagram(diagram) => {
                let rendered = renderer::render_diagram(&diagram, options);
                for l in rendered.lines() {
                    self.lines.push(l.to_string());
                }
            }
        }
        self.truncate_history(options.limits.max_pager_lines);
    }

    pub fn truncate_history(&mut self, max_lines: usize) {
        if max_lines == 0 {
            self.lines.clear();
            self.scroll = 0;
            return;
        }

        let excess = self.lines.len().saturating_sub(max_lines);
        if excess > 0 {
            self.lines.drain(0..excess);
            self.scroll = self.scroll.saturating_sub(excess);
        }
    }

    pub const fn scroll_up(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, delta: usize) {
        let max_scroll = self.lines.len().saturating_sub(self.visible_height);
        self.scroll = (self.scroll + delta).min(max_scroll);
        if self.scroll >= max_scroll {
            self.auto_scroll = true;
        }
    }

    pub fn update_scroll_to_bottom(&mut self, max_visible: usize) {
        self.visible_height = max_visible;
        if self.auto_scroll {
            self.scroll = self.lines.len().saturating_sub(max_visible);
        }
    }
}

/// Lance le pager interactif TUI temps réel.
///
/// # Errors
/// Renvoie `CliError::TerminalInit` ou `CliError::Io`.
pub fn run_pager<R: BufRead + Send + 'static>(
    reader: R,
    options: RenderOptions,
) -> Result<(), CliError> {
    enable_raw_mode().map_err(|e| CliError::TerminalInit(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| CliError::TerminalInit(e.to_string()))?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| CliError::TerminalInit(e.to_string()))?;

    let rx = spawn_reader_thread(reader, options.limits);
    let mut app = PagerApp::new();

    run_event_loop(&mut terminal, &mut app, &rx, options)
}

/// Démarre le thread d'arrière-plan de lecture de flux.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PagerEvent {
    Item(StreamItem),
    Finished,
}

fn spawn_reader_thread<R: BufRead + Send + 'static>(
    reader: R,
    limits: crate::domain::ResourceLimits,
) -> Receiver<Result<PagerEvent, CliError>> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut machine = StreamStateMachine::with_limits(limits);
        for line_result in reader.lines() {
            let line = match line_result {
                Ok(line) => line,
                Err(err) => {
                    let _ = tx.send(Err(CliError::Io(err.to_string())));
                    return;
                }
            };

            match machine.process_line(&line) {
                Ok(Some(item)) => {
                    if tx.send(Ok(PagerEvent::Item(item))).is_err() {
                        return;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    let _ = tx.send(Err(err));
                    return;
                }
            }
        }
        if let Some(final_item) = machine.finish() {
            let _ = tx.send(Ok(PagerEvent::Item(final_item)));
        }
        let _ = tx.send(Ok(PagerEvent::Finished));
    });
    rx
}

/// Boucle principale de rendu et de gestion des événements TUI.
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut PagerApp,
    rx: &Receiver<Result<PagerEvent, CliError>>,
    options: RenderOptions,
) -> Result<(), CliError> {
    loop {
        consume_pending_items(app, rx, options)?;

        terminal
            .draw(|frame| render_frame(frame, app))
            .map_err(|e| CliError::Io(e.to_string()))?;

        if matches!(event::poll(Duration::from_millis(50)), Ok(true)) && poll_and_handle_key(app) {
            break;
        }
    }
    Ok(())
}

/// Lit et traite un événement clavier s'il est disponible.
fn poll_and_handle_key(app: &mut PagerApp) -> bool {
    if let Ok(Event::Key(key)) = event::read() {
        key.kind == KeyEventKind::Press && handle_key_event(app, key.code)
    } else {
        false
    }
}

/// Consomme les éléments reçus du thread de streaming.
fn consume_pending_items(
    app: &mut PagerApp,
    rx: &Receiver<Result<PagerEvent, CliError>>,
    options: RenderOptions,
) -> Result<(), CliError> {
    loop {
        match rx.try_recv() {
            Ok(Ok(PagerEvent::Item(item))) => app.add_item(item, options),
            Ok(Ok(PagerEvent::Finished)) => app.stream_finished = true,
            Ok(Err(err)) => return Err(err),
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => {
                app.stream_finished = true;
                return Ok(());
            }
        }
    }
}

/// Traite les frappes clavier. Retourne `true` pour quitter.
fn handle_key_event(app: &mut PagerApp, code: KeyCode) -> bool {
    let page_delta = app.visible_height.max(1);
    match code {
        KeyCode::Char('q') | KeyCode::Esc => true,
        KeyCode::Up | KeyCode::Char('k') => {
            app.scroll_up(1);
            false
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.scroll_down(1);
            false
        }
        KeyCode::PageUp => {
            app.scroll_up(page_delta);
            false
        }
        KeyCode::PageDown => {
            app.scroll_down(page_delta);
            false
        }
        _ => false,
    }
}

/// Rendu graphique du frame TUI.
fn render_frame(frame: &mut Frame, app: &mut PagerApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let visible_height = chunks[0].height as usize;
    app.update_scroll_to_bottom(visible_height);

    let display_lines: Vec<Line> = app
        .lines
        .iter()
        .skip(app.scroll)
        .take(visible_height)
        .map(|l| Line::from(l.as_str()))
        .collect();

    let title = if app.stream_finished {
        " Markdown Pager (Terminé) "
    } else {
        " Markdown Pager (Streaming...) "
    };

    let paragraph =
        Paragraph::new(display_lines).block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(paragraph, chunks[0]);

    let available_cols = chunks[1].width as usize;
    let status_text =
        format_status_bar(app.scroll, app.lines.len(), app.auto_scroll, available_cols);
    let status_bar = Paragraph::new(Line::from(vec![Span::styled(
        status_text,
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )]));

    frame.render_widget(status_bar, chunks[1]);
}

fn format_status_bar(
    scroll: usize,
    total_lines: usize,
    auto_scroll: bool,
    available_cols: usize,
) -> String {
    let full = format!(
        " Ligne {}/{} | 'q': Quitter | Fleches/PageUp/PageDown: Défiler | Auto-scroll: {} ",
        scroll + 1,
        total_lines.max(1),
        if auto_scroll { "ON" } else { "OFF" }
    );
    if ViewportGeometry::display_width(&full) <= available_cols {
        return full;
    }

    let compact = format!(
        " L.{}/{} | 'q': Quitter | Auto: {} ",
        scroll + 1,
        total_lines.max(1),
        if auto_scroll { "ON" } else { "OFF" }
    );
    if ViewportGeometry::display_width(&compact) <= available_cols {
        compact
    } else {
        ViewportGeometry::new(u16::try_from(available_cols).unwrap_or(0), 1).fit_line(&compact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GraphicsProtocol, ResourceLimits, ThemeMode, ViewportGeometry};
    use std::io::Cursor;

    #[test]
    fn test_pager_app_history_truncation() {
        let mut app = PagerApp::new();
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            ResourceLimits {
                max_pager_lines: 3,
                ..ResourceLimits::default()
            },
        );

        app.add_item(StreamItem::Text("line 1".to_string()), options);
        app.add_item(StreamItem::Text("line 2".to_string()), options);
        app.add_item(StreamItem::Text("line 3".to_string()), options);
        app.add_item(StreamItem::Text("line 4".to_string()), options);

        assert_eq!(app.lines.len(), 3);
        assert_eq!(app.lines, vec!["line 2", "line 3", "line 4"]);
    }

    #[test]
    fn test_pager_scroll_navigation_with_dynamic_height() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.update_scroll_to_bottom(10);
        assert_eq!(app.scroll, 40);

        app.scroll_up(5);
        assert_eq!(app.scroll, 35);
        assert!(!app.auto_scroll);

        app.scroll_down(2);
        assert_eq!(app.scroll, 37);

        assert!(!handle_key_event(&mut app, KeyCode::PageUp));
        assert_eq!(app.scroll, 27);

        assert!(!handle_key_event(&mut app, KeyCode::PageDown));
        assert_eq!(app.scroll, 37);

        assert!(handle_key_event(&mut app, KeyCode::Char('q')));
    }

    #[test]
    fn test_reader_thread_and_stream_finished() {
        let input = "line A\nline B\n";
        let cursor = Cursor::new(input);
        let rx = spawn_reader_thread(cursor, ResourceLimits::default());

        let mut app = PagerApp::new();
        let options = RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
        );

        let res = consume_pending_items(&mut app, &rx, options);
        assert!(res.is_ok());

        thread::sleep(Duration::from_millis(50));
        let res2 = consume_pending_items(&mut app, &rx, options);
        assert!(res2.is_ok());
        assert!(app.stream_finished);
        assert_eq!(app.lines, vec!["line A", "line B"]);
    }

    #[test]
    fn test_reader_thread_propagates_limit_error() {
        let input = "```mermaid\noversized diagram\n```\n";
        let cursor = Cursor::new(input);
        let limits = ResourceLimits {
            max_diagram_bytes: 5,
            ..ResourceLimits::default()
        };
        let rx = spawn_reader_thread(cursor, limits);

        let mut app = PagerApp::new();
        let options = RenderOptions::with_limits(
            ThemeMode::Dark,
            GraphicsProtocol::HalfBlocks,
            ViewportGeometry::new(80, 24),
            limits,
        );

        thread::sleep(Duration::from_millis(50));
        let res = consume_pending_items(&mut app, &rx, options);
        assert!(matches!(res, Err(CliError::ResourceLimit(_))));
    }

    #[test]
    fn test_format_status_bar_adapts_to_visual_width() {
        let full = format_status_bar(0, 100, true, 100);
        assert!(full.contains("Fleches/PageUp/PageDown"));
        assert!(ViewportGeometry::display_width(&full) <= 100);

        let compact = format_status_bar(0, 100, true, 45);
        assert!(compact.contains("L.1/100"));
        assert!(!compact.contains("Fleches/PageUp/PageDown"));
        assert!(ViewportGeometry::display_width(&compact) <= 45);

        let narrow = format_status_bar(0, 100, true, 15);
        assert!(ViewportGeometry::display_width(&narrow) <= 15);
    }
}
