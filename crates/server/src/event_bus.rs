//! The in-process event bus. Publishing never blocks; subscribers each get their own cursor.

use massorca_protocol::methods::events::ChangeEvent;
use tokio::sync::broadcast;

/// Events a subscriber may fall behind by before the oldest are dropped for it.
const EVENT_CAPACITY: usize = 1024;

/// Change events published by the service. Delivery is best effort: a subscriber that falls more than `EVENT_CAPACITY` events behind skips the ones it missed (logged), so clients re-list to resynchronise.
#[derive(Clone, Debug)]
pub struct EventBus {
    sender: broadcast::Sender<ChangeEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(EVENT_CAPACITY);
        Self { sender }
    }

    /// Publishes to every current subscriber. With no subscribers the event is dropped, which is fine: nobody is listening.
    pub fn publish(&self, event: ChangeEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> EventSubscription {
        EventSubscription {
            receiver: self.sender.subscribe(),
        }
    }
}

/// One subscriber. Dropping it unsubscribes.
#[derive(Debug)]
pub struct EventSubscription {
    receiver: broadcast::Receiver<ChangeEvent>,
}

impl EventSubscription {
    /// The next event, or `None` once the bus is gone. Skipped events are logged rather than returned.
    pub async fn next(&mut self) -> Option<ChangeEvent> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => return Some(event),
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(
                        skipped,
                        "an event subscriber fell behind and skipped events"
                    );
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}
