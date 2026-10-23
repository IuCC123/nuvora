//! `nuvora`: one binary. `nuvora` (serve) or `nuvora openapi` (print the spec,
//! used by `bun run gen:api` to regenerate the TypeScript client).

mod api;
mod auth;
mod config;
mod discovery;
mod net;
mod party;
mod transcode;
mod web;

use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
            let spec = api::openapi();
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
    let encoder = transcode::pick_encoder(&tools, config.hardware_encoding).await;
    let cores = std::thread::available_parallelism().map_or(2, |n| n.get());
    let probe_concurrency =
        NonZeroUsize::new(cores.min(MAX_PROBE_CONCURRENCY)).expect("at least one");
    let library = Library::open(LibraryConfig {
        data_dir: config.data_dir.clone(),
        libraries: config.libraries,
        tools: tools.clone(),
        probe_concurrency,
        settle_window: SETTLE_WINDOW,
        online_metadata: true,
    })
    .await?;
    tracing::info!(data_dir = %config.data_dir.display(), "catalog open");

    let listener = TcpListener::bind(config.bind).await?;
    let bind = listener.local_addr()?;
    let id = library.server_id().await?;
    let announcer = if config.discovery {
        let name = library.server_name().await?;
        let name = name.as_deref().unwrap_or(api::DEFAULT_SERVER_NAME);
        discovery::Announcer::start(&id, name, bind)
    } else {
        None
    };
    let state = api::AppState {
        library: library.clone(),
        auth: Arc::new(auth::Auth::new(net::TrustedProxies::new(
            config.trusted_proxies,
        ))),
        transcoder: transcode::Transcoder::new(tools.clone(), encoder.clone(), &config.data_dir),
        parties: Arc::new(party::Parties::default()),
        server: Arc::new(api::ServerSetup {
            id,
            announcer,
            data_dir: config.data_dir,
            bind,
            tools,
            encoder,
            started: Instant::now(),
        }),
    };
    let server = state.server.clone();
    let parties = state.parties.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(15));
        loop {
            tick.tick().await;
            parties.tidy();
        }
    });
    let app = api::router(state)
        .fallback(web::serve_app)
        .layer(api::compression());
    tracing::info!("listening on http://{bind}");
    library.request_scan();
    // The peer address keys sign-in rate limiting.
    let app = app.into_make_service_with_connect_info::<std::net::SocketAddr>();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    if let Some(announcer) = &server.announcer {
        announcer.stop();
    }
    Ok(())
}

/// Ctrl-C, or SIGTERM: `docker stop` sends SIGTERM through tini.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::warn!("cannot listen for SIGTERM: {e}");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutting down");
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;
