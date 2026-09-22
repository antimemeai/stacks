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
    let library_path =
        std::env::var("STACKS_LIBRARY_DB").unwrap_or_else(|_| "data/library.db".to_string());
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
    let library = match stacks_core::library::LibraryStore::open_read_only(&library_path) {
        Ok(l) => Some(l),
        Err(e) => {
            eprintln!("library db {library_path} unavailable ({e}); library endpoints disabled");
            None
        }
    };
    let cache_dir = std::env::var("STACKS_FASTEMBED_CACHE")
        .unwrap_or_else(|_| "/home/patrick/neurotic_library/.fastembed_cache".to_string());
    let semantic = if library.is_some() {
        match stacks_api::semantic::FastEmbedder::new(std::path::Path::new(&cache_dir)) {
            Ok(e) => Some(Box::new(e) as Box<dyn stacks_api::semantic::Embedder>),
            Err(e) => {
                eprintln!("embedding model unavailable ({e}); semantic endpoint disabled");
                None
            }
        }
    } else {
        None
    };
    let app = stacks_api::build_app_semantic(store, library, semantic);
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
