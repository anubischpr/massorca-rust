//! A dropped `LocalStream` closes without waiting for its peer to read, and the peer still reads what it wrote.
#![cfg(windows)]

use std::time::Duration;

use interprocess::local_socket::tokio::{Listener, Stream};
use interprocess::local_socket::traits::tokio::{Listener as _, Stream as _};
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Name, ToNsName};
use massorca_profile::LocalStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;

const READ_WAIT: Duration = Duration::from_secs(10);

fn pipe_name() -> Name<'static> {
    format!("massorca-local-stream-test-{}", std::process::id())
        .to_ns_name::<GenericNamespaced>()
        .expect("a pipe name")
}

async fn accepted(listener: &Listener) -> (Stream, Stream) {
    let (client, server) = tokio::join!(Stream::connect(pipe_name()), listener.accept());
    (
        client.expect("the client connects"),
        server.expect("the server accepts"),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_dropped_stream_keeps_its_unread_bytes_and_closes_although_another_peer_never_reads() {
    let listener = ListenerOptions::new()
        .name(pipe_name())
        .create_tokio()
        .expect("the listener binds");

    // A plain stream left with writes its peer never reads keeps interprocess's one closing thread busy for good.
    let (_never_reads, stuck) = accepted(&listener).await;
    let flood = tokio::spawn(async move {
        let mut send: &Stream = &stuck;
        let chunk = vec![0_u8; 1024 * 1024];
        loop {
            if send.write_all(&chunk).await.is_err() {
                return;
            }
        }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    flood.abort();
    let _ = flood.await;

    let (mut client, server) = accepted(&listener).await;
    let server = LocalStream::new(server);
    let mut send: &Stream = server.get();
    send.write_all(b"last words").await.expect("the write");
    drop(server);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let mut read = Vec::new();
    timeout(READ_WAIT, client.read_to_end(&mut read))
        .await
        .expect("the close arrives although another peer never reads")
        .expect("the read");
    assert_eq!(read, b"last words");
}
