use tokio_tungstenite::tungstenite;

/// Why a network operation failed. Neither `Debug` nor `Display` ever carries key bytes.
#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("network i/o failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls failed: {0}")]
    Tls(#[from] rustls::Error),
    #[error("websocket failed: {0}")]
    WebSocket(#[from] Box<tungstenite::Error>),
    #[error("invalid certificate or key: {0}")]
    Certificate(String),
    #[error("invalid pin: {0}")]
    InvalidPin(String),
    #[error("the server's certificate does not match the pinned key")]
    ServerPinMismatch,
    #[error("no address to connect to")]
    NoAddress,
    #[error("timed out")]
    Timeout,
}

impl From<tungstenite::Error> for NetError {
    fn from(error: tungstenite::Error) -> Self {
        Self::WebSocket(Box::new(error))
    }
}
