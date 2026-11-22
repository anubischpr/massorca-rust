//! The one store the shell derives its view models from (decision 0028). Sample data fills it once
//! and never changes it; the client source changes it only through the updates the bridge sends.

use std::io;
use std::time::Instant;

use gpui::{Context, Task};
use massorca_client::Autostart;
use massorca_profile::Profile;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::data::DataSource;
use crate::model::service_view::{NOTICE_TICK, service_snapshot};
use crate::model::shell::ShellSnapshot;
use crate::service::{ServiceBridge, ServiceHandle, StoreState, StoreUpdate, connect_options};

/// Where the store's data comes from.
#[derive(Clone, Debug)]
enum Source {
    /// A fixed snapshot, shown as it is.
    Sample(Box<ShellSnapshot>),
    /// What a service reported, held in `state`.
    Service,
}

/// The sample source's counterpart on the client: starts the bridge and hands out what the shell
/// needs to follow it. Its `DataSource` snapshot is the first paint, before anything connected.
#[derive(Debug)]
pub struct ClientDataSource {
    bridge: ServiceBridge,
}

impl ClientDataSource {
    /// Starts the bridge for `profile`; the service is started too when `autostart` allows it.
    pub fn start(profile: Profile, autostart: Option<Autostart>) -> io::Result<Self> {
        Ok(Self {
            bridge: ServiceBridge::start(profile, connect_options(autostart))?,
        })
    }

    pub fn handle(&self) -> ServiceHandle {
        self.bridge.handle()
    }

    /// Splits off the bridge and its updates. The bridge must outlive the store.
    pub fn into_parts(mut self) -> (ServiceBridge, UnboundedReceiver<StoreUpdate>) {
        let updates = self
            .bridge
            .take_updates()
            .expect("the updates are taken once, here");
        (self.bridge, updates)
    }
}

impl DataSource for ClientDataSource {
    fn snapshot(&self) -> ShellSnapshot {
        service_snapshot(&StoreState::default(), Instant::now())
    }
}

/// The projects and workspaces the shell shows, and how the link to the service stands.
#[derive(Debug)]
pub struct ServiceStore {
    source: Source,
    state: StoreState,
    /// Keeps the foreground task that applies the bridge's updates.
    _follow: Option<Task<()>>,
    /// Redraws the retry timer while the link is down.
    ticker: Option<Task<()>>,
}

impl ServiceStore {
    /// A store filled once from sample data.
    pub fn sample(data: &impl DataSource) -> Self {
        Self {
            source: Source::Sample(Box::new(data.snapshot())),
            state: StoreState::default(),
            _follow: None,
            ticker: None,
        }
    }

    /// A store that applies the bridge's updates as they arrive. Updates that arrive together are
    /// applied together and notify once.
    pub fn following(mut updates: UnboundedReceiver<StoreUpdate>, cx: &mut Context<Self>) -> Self {
        let follow = cx.spawn(async move |this, cx| {
            while let Some(first) = updates.recv().await {
                let mut batch = vec![first];
                while let Ok(more) = updates.try_recv() {
                    batch.push(more);
                }
                if this.update(cx, |store, cx| store.apply(batch, cx)).is_err() {
                    break;
                }
            }
        });
        Self {
            source: Source::Service,
            state: StoreState::default(),
            _follow: Some(follow),
            ticker: None,
        }
    }

    pub fn is_sample(&self) -> bool {
        matches!(self.source, Source::Sample(_))
    }

    pub fn state(&self) -> &StoreState {
        &self.state
    }

    /// The view model the shell draws at `now`.
    pub fn snapshot(&self, now: Instant) -> ShellSnapshot {
        match &self.source {
            Source::Sample(snapshot) => (**snapshot).clone(),
            Source::Service => service_snapshot(&self.state, now),
        }
    }

    /// Folds a batch of updates in and notifies once.
    pub fn apply(&mut self, updates: Vec<StoreUpdate>, cx: &mut Context<Self>) {
        self.apply_quietly(updates);
        self.keep_time_fresh(cx);
        cx.notify();
    }

    /// Folds updates in without notifying observers. A sample shell's observer would take the
    /// sample snapshot again, so the debug launch seeds its start page this way.
    pub fn apply_quietly(&mut self, updates: Vec<StoreUpdate>) {
        let now = Instant::now();
        for update in updates {
            self.state.apply(update, now);
        }
    }

    /// While the link is not up the notice shows a timer and an age, so redraw every second.
    fn keep_time_fresh(&mut self, cx: &mut Context<Self>) {
        if self.state.connection.is_connected() {
            self.ticker = None;
            return;
        }
        if self.ticker.is_some() {
            return;
        }
        self.ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(NOTICE_TICK).await;
                let keep = this
                    .update(cx, |store, cx| {
                        let down = !store.state.connection.is_connected();
                        if down {
                            cx.notify();
                        } else {
                            store.ticker = None;
                        }
                        down
                    })
                    .unwrap_or(false);
                if !keep {
                    break;
                }
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::service::ConnectionState;
    use crate::service::fixtures::{project, workspace};
    use gpui::{AppContext, TestAppContext};
    use std::cell::Cell;
    use std::rc::Rc;
    use tokio::sync::mpsc::unbounded_channel;

    #[gpui::test]
    fn a_sample_store_shows_the_sample_snapshot_unchanged(cx: &mut TestAppContext) {
        let store = cx.new(|_| ServiceStore::sample(&SampleDataSource));
        store.read_with(cx, |store, _| {
            assert!(store.is_sample());
            assert_eq!(store.snapshot(Instant::now()), SampleDataSource.snapshot());
        });
    }

    #[gpui::test]
    fn updates_that_arrive_together_are_applied_and_notify_once(cx: &mut TestAppContext) {
        let (tx, rx) = unbounded_channel();
        let up = ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        };
        tx.send(StoreUpdate::Connection(up))
            .expect("the store listens");
        tx.send(StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![workspace("w1", "p1", "one", 1)],
        })
        .expect("the store listens");
        tx.send(StoreUpdate::WorkspaceChanged(workspace(
            "w2", "p1", "two", 2,
        )))
        .expect("the store listens");
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        let notified = Rc::new(Cell::new(0));
        let seen = Rc::clone(&notified);
        cx.update(|cx| {
            cx.observe(&store, move |_, _| seen.set(seen.get() + 1))
                .detach();
        });
        cx.run_until_parked();
        assert_eq!(notified.get(), 1, "one notification for the whole batch");
        store.read_with(cx, |store, _| {
            assert!(!store.is_sample());
            assert_eq!(store.state().workspaces.len(), 2);
            let snapshot = store.snapshot(Instant::now());
            assert_eq!(snapshot.projects[0].hosts[0].workspaces.len(), 2);
        });
    }

    #[gpui::test]
    fn a_later_update_notifies_again_and_closing_the_channel_is_harmless(cx: &mut TestAppContext) {
        let (tx, rx) = unbounded_channel();
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        cx.run_until_parked();
        tx.send(StoreUpdate::ProjectChanged(project("p1", "alpha")))
            .expect("the store listens");
        cx.run_until_parked();
        store.read_with(cx, |store, _| assert_eq!(store.state().projects.len(), 1));
        drop(tx);
        cx.run_until_parked();
    }
}
