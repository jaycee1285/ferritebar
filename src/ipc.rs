use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::broadcast;

static IPC_TX: OnceLock<broadcast::Sender<String>> = OnceLock::new();

pub fn socket_path() -> PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(runtime_dir).join("ferritebar.sock")
}

fn sender() -> &'static broadcast::Sender<String> {
    IPC_TX.get_or_init(|| broadcast::channel(32).0)
}

pub fn subscribe() -> broadcast::Receiver<String> {
    sender().subscribe()
}

pub fn start_listener() {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let tx = sender().clone();
    crate::spawn(async move {
        use tokio::io::AsyncReadExt;
        use tokio::net::UnixListener;
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("IPC: failed to bind socket at {path:?}: {e}");
                return;
            }
        };
        tracing::debug!("IPC: listening on {path:?}");
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let mut buf = Vec::new();
                        if stream.read_to_end(&mut buf).await.is_ok() {
                            if let Ok(cmd) = String::from_utf8(buf) {
                                let cmd = cmd.trim().to_string();
                                if !cmd.is_empty() {
                                    let _ = tx.send(cmd);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::error!("IPC: accept error: {e}");
                }
            }
        }
    });
}

pub async fn send_msg(command: &str) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;
    let path = socket_path();
    let mut stream = UnixStream::connect(&path).await?;
    stream.write_all(command.as_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}
