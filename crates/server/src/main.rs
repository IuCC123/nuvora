//! `nuvora`: one binary. `nuvora` (serve) or `nuvora openapi` (print the spec,
//! used by `bun run gen:api` to regenerate the TypeScript client).

mod api;
mod config;
mod web;

use std::num::NonZeroUsize;
use std::time::Duration;

use clap::{Parser, Subcommand};
use nuvora_library::{Library, LibraryConfig};
use nuvora_media::MediaTools;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "nuvora", version, about = "Nuvora media server")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    serve: config::ServeArgs,
}

#[derive(Subcommand)]
enum Command {
    /// Run the server (the default).
    Serve,
    /// Print the OpenAPI document as JSON and exit.
    Openapi,
}

/// Files modified more recently than this are left for the next scan: they are
/// probably still being copied onto the share.
const SETTLE_WINDOW: Duration = Duration::from_secs(60);
const MAX_PROBE_CONCURRENCY: usize = 8;

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve) {
        Command::Openapi => {
            let (_, spec) = api::router();
            println!("{}", spec.to_pretty_json().expect("spec serializes"));
            Ok(())
        }
        Command::Serve => serve(cli.serve).await,
    }
}

/// Startup order is chosen for first paint: open the catalog (fast), bind and
/// serve immediately from whatever is already cataloged, and only then kick off
/// the startup scan in the background.
async fn serve(args: config::ServeArgs) -> Result<(), BoxError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let config = config::load(&args)?;
    let tools = MediaTools::locate(config.ffprobe.clone(), config.ffmpeg.clone()).await?;
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    let probe_concurrency =
        NonZeroUsize::new(cores.min(MAX_PROBE_CONCURRENCY)).expect("at least one");
    let library = Library::open(LibraryConfig {
        data_dir: config.data_dir.clone(),
        libraries: config.libraries,
        tools,
        probe_concurrency,
        settle_window: SETTLE_WINDOW,
    })
    .await?;
    tracing::info!(data_dir = %config.data_dir.display(), "catalog open");

    let (api, _) = api::router();
    let app = api
        .with_state(library.clone())
        .fallback(web::serve_app)
        .layer(api::compression());
    let listener = TcpListener::bind(config.bind).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    library.request_scan();
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await?;
    Ok(())
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;
