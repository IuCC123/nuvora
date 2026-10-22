//! `nuvora`: one binary. `nuvora` (serve) or `nuvora openapi` (print the spec,
//! used by `bun run gen:api` to regenerate the TypeScript client).

mod api;
mod config;
mod web;

use clap::{Parser, Subcommand};

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
    let _ = args;
    // TODO:
    // tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).init();
    // let config = config::load(&args)?;                    // validated; warns if bind is not loopback
    // let tools = MediaTools::locate(config.ffprobe, config.ffmpeg).await?;
    // let library = Library::open(LibraryConfig { data_dir, libraries, tools, probe_concurrency }).await?;
    // let (api, _) = api::router();
    // let app = api.with_state(library.clone()).fallback(web::serve_app).layer(api::compression());
    // let listener = TcpListener::bind(config.bind).await?;
    // library.request_scan();
    // axum::serve(listener, app).with_graceful_shutdown(ctrl_c()).await?;
    todo!()
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;
