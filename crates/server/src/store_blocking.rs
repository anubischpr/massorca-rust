//! Runs synchronous store calls on the blocking pool, so the async service never waits on SQLite.

use massorca_core_store::{Store, StoreError};
use massorca_protocol::{ErrorBody, codes};

use crate::context::AppState;

/// Runs `call` against the store. The call counts as activity until it returns, even if its caller is aborted, because
/// a blocking call cannot be cancelled once it has started.
pub(crate) async fn run<T, F>(state: &AppState, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Store) -> Result<T, StoreError> + Send + 'static,
{
    let store = state.store.clone();
    let work = state.activity.enter();
    match tokio::task::spawn_blocking(move || {
        let _work = work;
        call(&store)
    })
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(ErrorBody::new(codes::INTERNAL, error.to_string())),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the store task failed: {error}"),
        )),
    }
}
