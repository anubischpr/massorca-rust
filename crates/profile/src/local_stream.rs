//! A connection on the service endpoint whose close never waits for the peer to read.

use interprocess::local_socket::tokio::Stream;

/// Owns a local socket stream for a transport's reader and writer.
///
/// interprocess hands a written pipe that is dropped to one shared thread that waits, pipe after pipe, for each peer to
/// read everything; one peer that never reads then stalls every later close in the process. Completed writes already
/// sit in the pipe and stay readable after the close, so dropping skips that wait.
#[derive(Debug)]
pub struct LocalStream(Stream);

impl LocalStream {
    pub fn new(stream: Stream) -> Self {
        Self(stream)
    }

    /// The stream; `&Stream` reads and writes.
    pub fn get(&self) -> &Stream {
        &self.0
    }
}

impl Drop for LocalStream {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let Stream::NamedPipe(pipe) = &self.0;
            pipe.inner().assume_flushed();
        }
    }
}
