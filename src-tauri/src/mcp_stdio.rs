use serde::Deserialize;
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Clone, Deserialize)]
struct Connection {
    port: u16,
    token: String,
    pid: u32,
}
fn connection_file() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = std::env::var_os("AI_EXCALIDRAW_BRIDGE_FILE") {
        return Ok(path.into());
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or("Cannot locate the user home directory")?;
    #[cfg(target_os = "macos")]
    let root = PathBuf::from(home).join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map_or_else(
        || PathBuf::from(home).join("AppData/Roaming"),
        PathBuf::from,
    );
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let root = std::env::var_os("XDG_DATA_HOME")
        .map_or_else(|| PathBuf::from(home).join(".local/share"), PathBuf::from);
    Ok(root.join("com.local.ai-excalidraw/bridge.json"))
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let path = connection_file()?;
    let connection: Connection = serde_json::from_slice(
        &std::fs::read(&path)
            .map_err(|_| "Open AI Excalidraw desktop first; its MCP service is not running")?,
    )?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()?;
    let url = format!("http://127.0.0.1:{}/mcp", connection.port);
    let finished = Arc::new(AtomicBool::new(false));
    let monitor_finished = Arc::clone(&finished);
    let current = connection.clone();
    std::thread::spawn(move || {
        while !monitor_finished.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(500));
            let valid = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Connection>(&bytes).ok())
                .is_some_and(|live| {
                    live.pid == current.pid
                        && live.port == current.port
                        && live.token == current.token
                });
            let address = std::net::SocketAddr::from(([127, 0, 0, 1], current.port));
            if !valid
                || std::net::TcpStream::connect_timeout(&address, Duration::from_millis(300))
                    .is_err()
            {
                if !monitor_finished.load(Ordering::Acquire) {
                    eprintln!("AI Excalidraw desktop closed; MCP connector stopped.");
                    std::process::exit(0);
                }
                return;
            }
        }
    });
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let input = std::io::stdin();
        let mut input = input.lock();
        let mut output = std::io::stdout().lock();
        loop {
            // Bound each frame without allocating an unbounded newline-free input.
            let mut bytes = Vec::new();
            let length =
                std::io::Read::take(&mut input, 2_097_153).read_until(b'\n', &mut bytes)?;
            if length == 0 {
                break;
            }
            if length > 2_097_152 {
                return Err("MCP frame is too large".into());
            }
            if bytes.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            let Some(value) = relay(&runtime, &client, &url, &connection.token, bytes)? else {
                continue;
            };
            serde_json::to_writer(&mut output, &value)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
        Ok(())
    })();
    finished.store(true, Ordering::Release);
    result
}

fn relay(
    runtime: &tokio::runtime::Runtime,
    client: &reqwest::Client,
    url: &str,
    token: &str,
    bytes: Vec<u8>,
) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
    let response = runtime
        .block_on(async {
            client
                .post(url)
                .bearer_auth(token)
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("mcp-protocol-version", "2025-11-25")
                .body(bytes)
                .send()
                .await
        })?
        .error_for_status()?;
    if response.status() == reqwest::StatusCode::ACCEPTED {
        return Ok(None);
    }
    let body = runtime.block_on(response.bytes())?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        reason = "Test setup and assertions fail on unexpected errors"
    )]
    use super::*;
    use std::io::Read;
    #[test]
    fn forwards_native_frame_from_sync_stdio_without_a_runtime_panic() {
        // Given a real loopback server, when synchronous stdio forwards a frame, then the wire response returns intact.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/mcp", listener.local_addr().unwrap());
        let responder = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut input = std::io::BufReader::new(socket.try_clone().unwrap());
            let mut length = 0;
            let mut authenticated = false;
            loop {
                let mut line = String::new();
                input.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                let line = line.to_ascii_lowercase();
                if line.starts_with("content-length:") {
                    length = line.split(':').nth(1).unwrap().trim().parse().unwrap();
                }
                if line == "authorization: bearer fixture-token\r\n" {
                    authenticated = true;
                }
            }
            let mut body = vec![0; length];
            input.read_exact(&mut body).unwrap();
            assert!(authenticated);
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&body).unwrap()["method"],
                "ping"
            );
            let reply = r#"{"jsonrpc":"2.0","id":1,"result":{}}"#;
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reply.len(),reply).unwrap();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        let request = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#.to_vec();
        let result = relay(&runtime, &client, &url, "fixture-token", request)
            .unwrap()
            .unwrap();
        assert_eq!(result["id"], 1);
        assert!(result["result"].is_object());
        responder.join().unwrap();
    }
}
