//! `terminal.attach`: a byte stream of JSON attach events and binary chunks (decision 0035). Needs `binary-frames/1`.
//! Dropping the stream detaches; a terminal that exits ends it after the `exited` event.

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::{AttachItem, AttachMode as CoreAttachMode, Attachment};
use massorca_protocol::methods::terminals::{
    self, AttachEvent, AttachMode, ChunkKind, MAX_CHUNK_BYTES, TerminalAttachParams, encode_chunk,
};
use massorca_protocol::{ErrorBody, capabilities};
use tokio::sync::mpsc;

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::terminals::{on_manager, terminal_of};
use crate::method::{ByteStreamSource, StreamItem};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// Items queued per attachment before the forwarder waits for the connection to catch up.
const QUEUE_CAPACITY: usize = 32;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register_byte_stream(
        Owner::Core,
        terminals::ATTACH,
        capabilities::TERMINALS,
        Permission::Workspace,
        attach,
    )?])
}

async fn attach(
    ctx: RequestCtx,
    params: TerminalAttachParams,
) -> Result<ByteStreamSource, ErrorBody> {
    let TerminalAttachParams {
        terminal,
        since,
        cols,
        rows,
        ..
    } = params;
    let manager = ctx.state.terminals.clone();
    let attachment = on_manager(move || {
        if cols.is_some() || rows.is_some() {
            let current = manager.get(&terminal)?;
            manager.resize(
                &terminal,
                PtySize {
                    cols: cols.unwrap_or(current.cols),
                    rows: rows.unwrap_or(current.rows),
                },
            )?;
        }
        manager.attach(&terminal, since)
    })
    .await?;

    let (items, source) = mpsc::channel(QUEUE_CAPACITY);
    tokio::spawn(forward(attachment, items));
    Ok(source)
}

/// Pushes the attachment's items until the client cancels, the terminal exits, or the connection goes away.
async fn forward(mut attachment: Attachment, items: mpsc::Sender<StreamItem>) {
    loop {
        let item = tokio::select! {
            () = items.closed() => return,
            item = attachment.next() => item,
        };
        let Some(item) = item else { return };
        if !send_item(&items, item).await {
            return;
        }
    }
}

/// Sends one item. `false` means the stream must end: the client is gone, or the terminal exited.
async fn send_item(items: &mpsc::Sender<StreamItem>, item: AttachItem) -> bool {
    match item {
        AttachItem::Attached { info, mode, offset } => {
            let mode = match mode {
                CoreAttachMode::Resume => AttachMode::Resume,
                CoreAttachMode::Snapshot => AttachMode::Snapshot,
            };
            let event = AttachEvent::Attached {
                terminal: terminal_of(info),
                mode,
                offset,
            };
            send_event(items, &event).await
        }
        AttachItem::Snapshot {
            offset,
            cols,
            rows,
            bytes,
        } => {
            let event = AttachEvent::Snapshot {
                offset,
                cols,
                rows,
                len: bytes.len() as u64,
            };
            if !send_event(items, &event).await {
                return false;
            }
            for part in bytes.chunks(MAX_CHUNK_BYTES) {
                let payload = encode_chunk(ChunkKind::Snapshot, offset, part);
                if items.send(StreamItem::Bytes(payload)).await.is_err() {
                    return false;
                }
            }
            true
        }
        AttachItem::Output { offset, bytes } => {
            for (index, part) in bytes.chunks(MAX_CHUNK_BYTES).enumerate() {
                let part_offset = offset.saturating_add((index * MAX_CHUNK_BYTES) as u64);
                let payload = encode_chunk(ChunkKind::Output, part_offset, part);
                if items.send(StreamItem::Bytes(payload)).await.is_err() {
                    return false;
                }
            }
            true
        }
        AttachItem::Resized { cols, rows } => {
            send_event(items, &AttachEvent::Resized { cols, rows }).await
        }
        AttachItem::Exited { info } => {
            // The stream ends after this item, so the caller stops even though the sender is still open.
            let event = AttachEvent::Exited {
                terminal: terminal_of(info),
            };
            send_event(items, &event).await;
            false
        }
    }
}

async fn send_event(items: &mpsc::Sender<StreamItem>, event: &AttachEvent) -> bool {
    match serde_json::to_value(event) {
        Ok(value) => items.send(StreamItem::Event(value)).await.is_ok(),
        Err(error) => {
            tracing::warn!(%error, "an attach event could not be encoded");
            false
        }
    }
}
