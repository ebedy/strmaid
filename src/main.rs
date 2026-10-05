use clap::Parser;
use crossterm::terminal;
use std::fs::File;
use std::io::{self, BufRead, BufReader, IsTerminal};
use std::process::ExitCode;
use strmaid::cli::{CliArgs, Subcommands};
use strmaid::doctor;
use strmaid::domain::{CliError, ExecutionMode, OutputFormat, ResourceLimits, ViewportGeometry};
use strmaid::filter;
use strmaid::mcp;
use strmaid::pager;
use strmaid::protocol;
use strmaid::pty;
use strmaid::renderer::RenderOptions;

fn main() -> ExitCode {
    let args = CliArgs::parse();
    match run_app(&args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("\x1b[31mErreur:\x1b[0m {err}");
            ExitCode::FAILURE
        }
    }
}

fn run_app(args: &CliArgs) -> Result<bool, CliError> {
    args.validate()?;

    if let Some(subcommand) = &args.command {
        return match subcommand {
            Subcommands::Doctor { format } => doctor::run_doctor(*format),
            Subcommands::Mcp => {
                mcp::run_mcp_server(io::stdin().lock(), io::stdout().lock())?;
                Ok(true)
            }
            Subcommands::Run { command } => {
                let options = build_render_options(args);
                let exit_code = pty::run_pty(command, options)?;
                Ok(exit_code == 0)
            }
        };
    }

    let options = build_render_options(args);

    if args.block_only {
        return execute_block_only(args, options);
    }

    let mode = determine_execution_mode(args);
    execute_stream(args, mode, options)
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

fn determine_execution_mode(args: &CliArgs) -> ExecutionMode {
    if args.format != OutputFormat::Human {
        ExecutionMode::StreamFilter
    } else if args.force_pager {
        ExecutionMode::LivePager
    } else if args.no_pager {
        ExecutionMode::StreamFilter
    } else if io::stdout().is_terminal() {
        ExecutionMode::LivePager
    } else {
        ExecutionMode::StreamFilter
    }
}

fn build_render_options(args: &CliArgs) -> RenderOptions {
    let protocol = protocol::detect_protocol(args.graphics);
    let theme = args.theme;

    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let effective_cols = args.width.unwrap_or(cols);
    let viewport = ViewportGeometry::new(effective_cols, rows);

    RenderOptions::with_all(
        theme,
        protocol,
        viewport,
        ResourceLimits::default(),
        args.format,
    )
    .with_auto_orient(!args.no_auto_orient)
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
