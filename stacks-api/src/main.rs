use std::net::SocketAddr;
use std::process::ExitCode;

use stacks_core::Store;

/// Listener construction is isolated here so swapping plain TCP for the
/// tsnet edge later touches only this function.
async fn bind(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(addr).await
}

#[tokio::main]
async fn main() -> ExitCode {
    let db_path = std::env::var("STACKS_DB").unwrap_or_else(|_| "data/stacks.db".to_string());
    let port: u16 = std::env::var("STACKS_API_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8432);
    let store = match Store::open_read_only(&db_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("open {db_path} read-only: {e}");
            return ExitCode::FAILURE;
        }
    };
    let app = stacks_api::build_app(store);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("stacks-api listening on http://{addr} (db: {db_path}, read-only)");
    match axum::serve(listener, app).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("serve: {e}");
            ExitCode::FAILURE
        }
    }
}
