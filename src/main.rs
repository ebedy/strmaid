use clap::Parser;
use crossterm::terminal;
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal};
use std::process::ExitCode;
use strmaid::cli::{CliArgs, Subcommands, TerminalContext};
use strmaid::doctor;
use strmaid::domain::{
    CliError, ExecutionMode, ResourceLimits, ViewportGeometry, sanitize_terminal_text,
};
use strmaid::filter;
use strmaid::mcp;
use strmaid::pager;
use strmaid::protocol;
use strmaid::pty;
use strmaid::renderer::RenderOptions;

/// Issue d'une exécution, convertie en code de sortie du processus.
enum AppOutcome {
    Success,
    Failure,
    /// Code de sortie d'un processus enfant (`strmaid run`), propagé tel quel.
    ChildExit(u32),
}

impl From<bool> for AppOutcome {
    fn from(success: bool) -> Self {
        if success {
            Self::Success
        } else {
            Self::Failure
        }
    }
}

impl From<AppOutcome> for ExitCode {
    fn from(outcome: AppOutcome) -> Self {
        match outcome {
            AppOutcome::Success => Self::SUCCESS,
            AppOutcome::Failure => Self::FAILURE,
            AppOutcome::ChildExit(code) => Self::from(u8::try_from(code).unwrap_or(u8::MAX)),
        }
    }
}

fn main() -> ExitCode {
    let args = CliArgs::parse();
    match run_app(&args) {
        Ok(outcome) => ExitCode::from(outcome),
        Err(err) => {
            let message = err.to_string();
            eprintln!(
                "\x1b[31mErreur:\x1b[0m {}",
                sanitize_terminal_text(&message)
            );
            exit_code_for(&err)
        }
    }
}

/// Code 2 pour une erreur d'usage de la ligne de commande (convention de `clap`),
/// 1 pour toute autre erreur d'exécution.
fn exit_code_for(err: &CliError) -> ExitCode {
    if matches!(err, CliError::CommandLine(_)) {
        ExitCode::from(2)
    } else {
        ExitCode::FAILURE
    }
}

fn run_app(args: &CliArgs) -> Result<AppOutcome, CliError> {
    args.validate()?;

    if let Some(subcommand) = &args.command {
        return match subcommand {
            Subcommands::Doctor { format } => doctor::run_doctor(*format).map(AppOutcome::from),
            Subcommands::Mcp => {
                mcp::run_mcp_server(io::stdin().lock(), io::stdout().lock())?;
                Ok(AppOutcome::Success)
            }
            Subcommands::Run { command } => {
                let options = build_render_options(args);
                pty::run_pty(command, options).map(AppOutcome::ChildExit)
            }
        };
    }

    let options = build_render_options(args);

    if args.block_only {
        return execute_block_only(args, options).map(AppOutcome::from);
    }

    let mode = args.execution_mode(TerminalContext {
        stdin_is_tty: io::stdin().is_terminal(),
        stdout_is_tty: io::stdout().is_terminal(),
    })?;
    execute_stream(args, mode, options).map(AppOutcome::from)
}

fn execute_block_only(args: &CliArgs, options: RenderOptions) -> Result<bool, CliError> {
    let stdout = io::stdout();
    if let Some(file_path) = &args.file {
        let file = File::open(file_path)
            .map_err(|e| CliError::Io(format!("{}: {e}", file_path.display())))?;
        let reader = BufReader::new(file);
        filter::run_block_only(reader, stdout.lock(), options)
    } else {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        filter::run_block_only(reader, stdout.lock(), options)
    }
}

fn execute_stream(
    args: &CliArgs,
    mode: ExecutionMode,
    options: RenderOptions,
) -> Result<bool, CliError> {
    if let Some(file_path) = &args.file {
        let file = File::open(file_path)
            .map_err(|e| CliError::Io(format!("{}: {e}", file_path.display())))?;
        let reader = BufReader::new(file);
        execute_with_reader(reader, mode, options)?;
    } else {
        let stdin = io::stdin();
        let reader = BufReader::new(stdin);
        execute_with_reader(reader, mode, options)?;
    }
    Ok(true)
}

fn build_render_options(args: &CliArgs) -> RenderOptions {
    let protocol = protocol::detect_protocol(args.graphics);
    let theme = args.theme;

    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let effective_cols = args.width.unwrap_or(cols);
    let viewport = ViewportGeometry::new(effective_cols, rows);

    let limits = if args.timeout_ms == 0 {
        ResourceLimits::default().with_render_timeout(None)
    } else {
        ResourceLimits::default()
            .with_render_timeout(Some(std::time::Duration::from_millis(args.timeout_ms)))
    };

    RenderOptions::with_all(theme, protocol, viewport, limits, args.format)
        .with_auto_orient(!args.no_auto_orient)
        .with_engine(args.engine)
        .with_fallback_asciibox(!args.no_fallback_asciibox)
        .with_raw_passthrough(args.raw_passthrough)
}

fn execute_with_reader<R: BufRead + Send + 'static>(
    reader: R,
    mode: ExecutionMode,
    options: RenderOptions,
) -> Result<(), CliError> {
    match mode {
        ExecutionMode::StreamFilter => {
            let stdout = io::stdout();
            filter::run_filter(reader, stdout.lock(), options)
        }
        ExecutionMode::LivePager => pager::run_pager(reader, options),
    }
}
