//! A loopback TCP relay in front of a paired host, counting the connections this computer opens to it. TLS passes
//! through untouched, so the host's key is checked as it would be without the relay.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

/// The relay's counts and its task; dropping it stops accepting.
pub struct HostProxy {
    pub address: SocketAddr,
    accepted: Arc<AtomicUsize>,
    open: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl HostProxy {
    /// Relays each connection made to `address` on to `target`.
    pub async fn start(target: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("the relay listens");
        let address = listener.local_addr().expect("the relay's address");
        let accepted = Arc::new(AtomicUsize::new(0));
        let open = Arc::new(AtomicUsize::new(0));
        let (counted, held) = (Arc::clone(&accepted), Arc::clone(&open));
        let task = tokio::spawn(async move {
            while let Ok((mut inbound, _)) = listener.accept().await {
                counted.fetch_add(1, Ordering::SeqCst);
                let held = Arc::clone(&held);
                tokio::spawn(async move {
                    held.fetch_add(1, Ordering::SeqCst);
                    if let Ok(mut outbound) = TcpStream::connect(target).await {
                        let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
                    }
                    held.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Self {
            address,
            accepted,
            open,
            task,
        }
    }

    /// Connections made to the host through the relay so far.
    pub fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    /// Connections through the relay that are open now.
    pub fn open(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }
}

impl Drop for HostProxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}
