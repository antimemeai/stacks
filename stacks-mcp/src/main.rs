use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let library_path =
        std::env::var("STACKS_LIBRARY_DB").unwrap_or_else(|_| "data/library.db".to_string());
    let port: u16 = std::env::var("STACKS_MCP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8433);
    let cache_dir = std::env::var("STACKS_FASTEMBED_CACHE")
        .unwrap_or_else(|_| "/home/patrick/neurotic_library/.fastembed_cache".to_string());
    let library = match stacks_core::library::LibraryStore::open_read_only(&library_path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("open {library_path} read-only: {e}");
            return ExitCode::FAILURE;
        }
    };
    let service = stacks_mcp::build_service_with_hosts(
        library,
        PathBuf::from(&cache_dir),
        std::env::var("STACKS_MCP_ALLOWED_HOSTS")
            .map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default(),
    );
    let app = axum::Router::new().nest_service("/mcp", service);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("bind {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("stacks-mcp listening on http://{addr}/mcp (library db: {library_path}, read-only)");
    match axum::serve(listener, app).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("serve: {e}");
            ExitCode::FAILURE
        }
    }
}
