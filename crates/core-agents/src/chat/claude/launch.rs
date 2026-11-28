//! How a Claude chat run starts, and the message a prompt becomes.

use serde_json::json;

use super::line_of;
use crate::chat::adapter::PromptImage;

/// The arguments a chat run of Claude Code starts with, before the kind's own: print mode with stream-json on both
/// streams, every frame (`--verbose`), partial messages for text deltas, and permission prompts as control requests.
pub const STREAM_JSON_ARGS: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--include-partial-messages",
    "--permission-prompt-tool",
    "stdio",
];

/// [`STREAM_JSON_ARGS`], then the arguments the kind's launch plan built.
pub(super) fn launch_args(planned: Vec<String>) -> Vec<String> {
    STREAM_JSON_ARGS
        .iter()
        .map(|&arg| arg.to_owned())
        .chain(planned)
        .collect()
}

/// The person's `text` as one stream-json user message line.
pub(super) fn user_message(text: &str) -> Vec<u8> {
    line_of(&json!({
        "type": "user",
        "message": { "role": "user", "content": [{ "type": "text", "text": text }] },
        "parent_tool_use_id": null,
        "session_id": "",
    }))
}

/// The person's `text` and `images` as one stream-json user message (decision 0172): the text block first, when there is
/// text, then one image block per image, its data base64 encoded with its media type.
pub(super) fn user_message_with_images(text: &str, images: &[PromptImage]) -> Vec<u8> {
    let mut content = Vec::with_capacity(images.len() + 1);
    if !text.is_empty() {
        content.push(json!({ "type": "text", "text": text }));
    }
    for image in images {
        content.push(json!({
            "type": "image",
            "source": { "type": "base64", "media_type": image.media_type, "data": image.data },
        }));
    }
    line_of(&json!({
        "type": "user",
        "message": { "role": "user", "content": content },
        "parent_tool_use_id": null,
        "session_id": "",
    }))
}
