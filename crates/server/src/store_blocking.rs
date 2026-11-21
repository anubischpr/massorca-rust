//! Runs synchronous store calls on the blocking pool, so the async service never waits on SQLite.

use massorca_core_store::{Store, StoreError};
use massorca_protocol::{ErrorBody, codes};

pub(crate) async fn run<T, F>(store: Store, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
{
    match tokio::task::spawn_blocking(move || call(&store)).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ErrorBody::new(codes::INTERNAL, error.to_string())),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the store task failed: {error}"),
        )),
    }
}
