mod ncm;
mod server;

use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let base = format!("http://{}", listener.local_addr()?);
    let state = Arc::new(Mutex::new(server::Sources::default()));
    let shutdown = CancellationToken::new();
    println!("{}", json!({"baseUrl": base}));
    std::io::stdout().flush()?;
    std::thread::spawn({
        let shutdown = shutdown.clone();
        let state = state.clone();
        move || {
            for line in std::io::stdin().lock().lines() {
                let Ok(line) = line else {
                    break;
                };
                let request: Value = match serde_json::from_str(&line) {
                    Ok(value) => value,
                    Err(_) => break,
                };
                let id = request["id"].clone();
                // Malformed third-party container parsing must not terminate the
                // RPC loop or leave a caller waiting forever.
                let result = std::panic::catch_unwind(|| server::command(&state, &base, &request))
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("malformed NCM container")));
                let response = match result {
                    Ok(value) => json!({"id": id, "result": value}),
                    Err(error) => json!({"id": id, "error": error.to_string()}),
                };
                println!("{response}");
                if std::io::stdout().flush().is_err() {
                    break;
                }
            }
            // Node owns stdin. EOF also covers forced termination of the parent.
            shutdown.cancel();
        }
    });
    tokio::select! {
        result = axum::serve(listener, server::router(state)) => result?,
        () = shutdown.cancelled() => {},
    }
    // Runtime teardown closes active HTTP bodies and releases the listener.
    Ok(())
}
