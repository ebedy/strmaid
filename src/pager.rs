use crate::domain::{
    CliError, GraphicsProtocol, ThemeMode, ViewportGeometry, sanitize_terminal_text,
};
use crate::renderer::{self, RenderOptions};
use crate::stream::{StreamItem, StreamStateMachine};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEvent, MouseEventKind,
    },
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
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        let _ = disable_raw_mode();
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

    /// Ajoute un élément du flux sous forme de lignes texte sans séquence terminale,
    /// `ratatui` n'interprétant ni les protocoles graphiques ni les SGR bruts.
    pub fn add_item(&mut self, item: StreamItem, options: RenderOptions) {
        let new_lines = match item {
            StreamItem::Text(text) => sanitized_lines(text.split('\n')),
            StreamItem::Diagram(diagram) => sanitized_lines(
                renderer::render_diagram(&diagram, pager_diagram_options(options)).lines(),
            ),
        };
        self.lines.extend(new_lines);
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

    pub const fn scroll_to_top(&mut self) {
        self.scroll = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = self.lines.len().saturating_sub(self.visible_height);
        self.auto_scroll = true;
    }

    pub fn scroll_half_page_up(&mut self) {
        let delta = (self.visible_height / 2).max(1);
        self.scroll_up(delta);
    }

    pub fn scroll_half_page_down(&mut self) {
        let delta = (self.visible_height / 2).max(1);
        self.scroll_down(delta);
    }

    pub fn update_scroll_to_bottom(&mut self, max_visible: usize) {
        self.visible_height = max_visible;
        if self.auto_scroll {
            self.scroll = self.lines.len().saturating_sub(max_visible);
        }
    }
}

fn sanitized_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    lines
        .map(|line| sanitize_terminal_text(line).into_owned())
        .collect()
}

/// Options de rendu des diagrammes dans le pager : tracé `AsciiBox` monochrome,
/// seul rendu représentable en `Line` ratatui.
const fn pager_diagram_options(options: RenderOptions) -> RenderOptions {
    RenderOptions {
        protocol: GraphicsProtocol::AsciiBox,
        theme: ThemeMode::Mono,
        ..options
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| CliError::TerminalInit(e.to_string()))?;
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

/// Résultat du traitement d'un événement terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalEventOutcome {
    NoEvent,
    Redraw,
    Quit,
}

/// Boucle principale de rendu et de gestion des événements TUI (Render-on-Change réactif).
fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &mut PagerApp,
    rx: &Receiver<Result<PagerEvent, CliError>>,
    options: RenderOptions,
) -> Result<(), CliError> {
    let mut dirty = true;

    loop {
        if consume_pending_items(app, rx, options)? {
            dirty = true;
        }

        if dirty {
            terminal
                .draw(|frame| render_frame(frame, app))
                .map_err(|e| CliError::Io(e.to_string()))?;
            dirty = false;
        }

        let poll_duration = if app.stream_finished {
            Duration::from_secs(3600)
        } else {
            Duration::from_millis(16)
        };

        if event::poll(poll_duration).map_err(|e| CliError::Io(e.to_string()))? {
            let event = event::read().map_err(|e| CliError::Io(e.to_string()))?;
            match process_terminal_event(app, &event) {
                TerminalEventOutcome::Quit => break,
                TerminalEventOutcome::Redraw => dirty = true,
                TerminalEventOutcome::NoEvent => {}
            }
        }
    }
    Ok(())
}

/// Traite un événement terminal (clavier, souris, redimensionnement).
fn process_terminal_event(app: &mut PagerApp, event: &Event) -> TerminalEventOutcome {
    match *event {
        Event::Key(key) => {
            if handle_key_press(app, key) {
                TerminalEventOutcome::Quit
            } else {
                TerminalEventOutcome::Redraw
            }
        }
        Event::Mouse(mouse) => {
            if handle_mouse_event(app, mouse) {
                TerminalEventOutcome::Redraw
            } else {
                TerminalEventOutcome::NoEvent
            }
        }
        Event::Resize(_, _) => TerminalEventOutcome::Redraw,
        _ => TerminalEventOutcome::NoEvent,
    }
}

/// Consomme les éléments reçus du thread de streaming.
/// Retourne `Ok(true)` si de nouveaux éléments ont été ajoutés ou si le statut a muté.
fn consume_pending_items(
    app: &mut PagerApp,
    rx: &Receiver<Result<PagerEvent, CliError>>,
    options: RenderOptions,
) -> Result<bool, CliError> {
    let mut changed = false;
    loop {
        match rx.try_recv() {
            Ok(Ok(PagerEvent::Item(item))) => {
                app.add_item(item, options);
                changed = true;
            }
            Ok(Ok(PagerEvent::Finished)) => {
                if !app.stream_finished {
                    app.stream_finished = true;
                    changed = true;
                }
            }
            Ok(Err(err)) => return Err(err),
            Err(TryRecvError::Empty) => return Ok(changed),
            Err(TryRecvError::Disconnected) => {
                if !app.stream_finished {
                    app.stream_finished = true;
                    changed = true;
                }
                return Ok(changed);
            }
        }
    }
}

/// Traite les combinaisons de touches avec modificateurs (`Ctrl`, etc.).
pub(crate) fn handle_key_press(app: &mut PagerApp, key: KeyEvent) -> bool {
    if key.kind != KeyEventKind::Press {
        return false;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') => return true,
            KeyCode::Char('u') => {
                app.scroll_half_page_up();
                return false;
            }
            KeyCode::Char('d') => {
                app.scroll_half_page_down();
                return false;
            }
            KeyCode::Char('b') => {
                app.scroll_up(app.visible_height.max(1));
                return false;
            }
            KeyCode::Char('f') => {
                app.scroll_down(app.visible_height.max(1));
                return false;
            }
            _ => return false,
        }
    }
    handle_key_event(app, key.code)
}

/// Traite les frappes clavier standards. Retourne `true` pour quitter.
pub(crate) fn handle_key_event(app: &mut PagerApp, code: KeyCode) -> bool {
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
        KeyCode::PageUp | KeyCode::Char('b') => {
            app.scroll_up(page_delta);
            false
        }
        KeyCode::PageDown | KeyCode::Char('f' | ' ') => {
            app.scroll_down(page_delta);
            false
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.scroll_to_top();
            false
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.scroll_to_bottom();
            false
        }
        KeyCode::Char('u') => {
            app.scroll_half_page_up();
            false
        }
        KeyCode::Char('d') => {
            app.scroll_half_page_down();
            false
        }
        _ => false,
    }
}

/// Traite les interactions de la molette de souris.
pub(crate) fn handle_mouse_event(app: &mut PagerApp, mouse: MouseEvent) -> bool {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            app.scroll_up(3);
            true
        }
        MouseEventKind::ScrollDown => {
            app.scroll_down(3);
            true
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

    fn kitty_options() -> RenderOptions {
        RenderOptions::new(
            ThemeMode::Dark,
            GraphicsProtocol::Kitty,
            ViewportGeometry::new(80, 24),
        )
    }

    #[test]
    fn test_pager_add_item_renders_diagram_as_plain_asciibox() {
        let mut app = PagerApp::new();
        let diagram =
            crate::domain::DiagramBlock::new("flowchart TD\n  A[Alpha] --> B[Beta]".to_string());
        app.add_item(StreamItem::Diagram(diagram), kitty_options());

        let joined = app.lines.join("\n");
        assert!(
            !joined.contains('\x1b'),
            "séquence ANSI résiduelle : {joined:?}"
        );
        assert!(joined.contains("Alpha") && joined.contains("Beta"));
        assert!(joined.contains('─'), "tracé AsciiBox attendu : {joined:?}");
    }

    #[test]
    fn test_pager_add_item_keeps_empty_markdown_lines() {
        let mut app = PagerApp::new();
        for text in ["titre", "", "suite"] {
            app.add_item(StreamItem::Text(text.to_string()), kitty_options());
        }
        assert_eq!(app.lines, vec!["titre", "", "suite"]);
    }

    #[test]
    fn test_pager_add_item_neutralizes_text_sequences() {
        let mut app = PagerApp::new();
        let text = "a\x1b]0;PWNED\x07b\x1b[31mc\x1b[0m".to_string();
        app.add_item(StreamItem::Text(text), kitty_options());
        assert_eq!(app.lines, vec!["abc"]);
    }

    #[test]
    fn test_pager_add_item_neutralizes_invalid_diagram_fallback() {
        let mut app = PagerApp::new();
        let diagram = crate::domain::DiagramBlock::new("xyz \x1b]52;c;eA==\x07 $$$".to_string());
        app.add_item(StreamItem::Diagram(diagram), kitty_options());
        assert!(app.lines.iter().all(|line| !line.contains('\x1b')));
        assert!(app.lines.iter().any(|line| line.contains("xyz")));
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

    #[test]
    fn test_pager_scroll_to_top_and_bottom() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.update_scroll_to_bottom(10);
        assert_eq!(app.scroll, 40);
        assert!(app.auto_scroll);

        app.scroll_to_top();
        assert_eq!(app.scroll, 0);
        assert!(!app.auto_scroll);

        app.scroll_to_bottom();
        assert_eq!(app.scroll, 40);
        assert!(app.auto_scroll);
    }

    #[test]
    fn test_pager_scroll_half_pages() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.visible_height = 20;
        app.scroll = 20;

        app.scroll_half_page_up();
        assert_eq!(app.scroll, 10);

        app.scroll_half_page_down();
        assert_eq!(app.scroll, 20);
    }

    #[test]
    fn test_handle_key_event_extended_keys() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.visible_height = 10;
        app.scroll = 20;

        // vim navigation
        assert!(!handle_key_event(&mut app, KeyCode::Char('k')));
        assert_eq!(app.scroll, 19);

        assert!(!handle_key_event(&mut app, KeyCode::Char('j')));
        assert_eq!(app.scroll, 20);

        // top / bottom
        assert!(!handle_key_event(&mut app, KeyCode::Char('g')));
        assert_eq!(app.scroll, 0);

        assert!(!handle_key_event(&mut app, KeyCode::Char('G')));
        assert_eq!(app.scroll, 40);

        // Space / f (page down) and b (page up)
        assert!(!handle_key_event(&mut app, KeyCode::Char('b')));
        assert_eq!(app.scroll, 30);

        assert!(!handle_key_event(&mut app, KeyCode::Char('f')));
        assert_eq!(app.scroll, 40);

        assert!(!handle_key_event(&mut app, KeyCode::Char(' ')));
        assert_eq!(app.scroll, 40);

        // half page u / d
        assert!(!handle_key_event(&mut app, KeyCode::Char('u')));
        assert_eq!(app.scroll, 35);

        assert!(!handle_key_event(&mut app, KeyCode::Char('d')));
        assert_eq!(app.scroll, 40);
    }

    #[test]
    fn test_handle_key_press_ctrl_combinations() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.visible_height = 10;
        app.scroll = 20;

        // Ctrl-c -> quit
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(handle_key_press(&mut app, ctrl_c));

        // Ctrl-u -> half page up
        let ctrl_u = KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert!(!handle_key_press(&mut app, ctrl_u));
        assert_eq!(app.scroll, 15);

        // Ctrl-d -> half page down
        let ctrl_d = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert!(!handle_key_press(&mut app, ctrl_d));
        assert_eq!(app.scroll, 20);

        // Release event -> ignored
        let mut release_key = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        release_key.kind = KeyEventKind::Release;
        assert!(!handle_key_press(&mut app, release_key));
    }

    #[test]
    fn test_handle_mouse_event() {
        let mut app = PagerApp::new();
        app.lines = (0..50).map(|i| format!("line {i}")).collect();
        app.visible_height = 10;
        app.scroll = 20;

        let mouse_up = MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        assert!(handle_mouse_event(&mut app, mouse_up));
        assert_eq!(app.scroll, 17);

        let mouse_down = MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        assert!(handle_mouse_event(&mut app, mouse_down));
        assert_eq!(app.scroll, 20);

        let mouse_moved = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 10,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        assert!(!handle_mouse_event(&mut app, mouse_moved));
    }

    #[test]
    fn test_process_terminal_event() {
        let mut app = PagerApp::new();

        let quit_key = Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert_eq!(
            process_terminal_event(&mut app, &quit_key),
            TerminalEventOutcome::Quit
        );

        let resize_event = Event::Resize(100, 40);
        assert_eq!(
            process_terminal_event(&mut app, &resize_event),
            TerminalEventOutcome::Redraw
        );

        let focus_lost = Event::FocusLost;
        assert_eq!(
            process_terminal_event(&mut app, &focus_lost),
            TerminalEventOutcome::NoEvent
        );
    }
}
