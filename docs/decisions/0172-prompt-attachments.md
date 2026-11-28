# 0172: Prompt attachments

Status: accepted, 2026-11-26.

A prompt can name up to 10 files of its session's workspace. A terminal run types a mention for each file, a chat run
mentions its text files and sends each image as an image block, and a file that leaves the workspace is refused before
anything is typed or sent. The capability is `agent-prompt-attachments/1`.

## Context

- A person wants the agent to read a file or look at a screenshot without pasting its contents into the prompt.
- A terminal run makes the agent read the file itself, and a chat image is read by the service, so a path that escaped
  the workspace would point either of them at any file the service's user can read. The check has to hold when the file
  is typed or sent, not only when the prompt is queued.

## Decision

| Topic | Decision |
|-------|----------|
| Wire | `agent.prompt` and `agent.queue` take `attachments: [{ "path": ... }]`, left out when empty. `PromptAttachment` has only `path`: the kind follows from the extension, so no `kind` field. |
| Capability | `agent-prompt-attachments/1`. The client refuses a request with attachments before sending it when the service does not offer the capability. |
| Path | Relative to the workspace root, or absolute when it is spelled under the root (as stored or canonical, case-insensitively on Windows and macOS). An absolute path is compared with the root before it is looked up on disk, so one elsewhere is refused as outside whether or not it exists. It is resolved with `regular_file_inside` in `massorca-core-workspace`, which follows symlinks and junctions and checks the file it reaches. |
| Refused paths | `..` anywhere; an absolute path not spelled under the root; a UNC, verbatim (`\\?\`) or device (`\\.\`) path, always; a link that reaches outside; a missing file; a directory or other entry; a name that is not UTF-8 text; a control character (C0, DEL or C1, newline, carriage return and tab included) in the path given or in the path the file is mentioned by. Each is `agent.attachment_invalid`, with the path in `data`. |
| Caps | `MAX_ATTACHMENTS` 10. `MAX_FILE_BYTES` 1 MiB for any file that is not an image. `MAX_IMAGE_BYTES` 3 MiB for an image, so its base64 form is 4.2 MB, under the 5 MB base64 limit of Amazon Bedrock (the first-party API allows 10 MB). Caps are `const`s in `crates/core-agents/src/attachments.rs`. |
| Image kinds | png, jpg, jpeg, gif and webp, by extension in any case. Other image formats (avif, bmp, heic, heif, ico, tif, tiff) are refused with `agent.attachment_unsupported`. Every image is refused on a terminal run, with the same code. |
| Terminal runs | The text is the prompt, then one mention per file, separated by spaces. The kind's `prompt.mention` template names each file, `{path}` standing for its path: `"@{path}"` for Claude. A kind without a template gets the plain paths on the prompt's own line, `look (attached: a.txt, b/c.md)`, or `(attached: ...)` alone when the text is empty: such a kind may type in plain paste mode, where each line break is an Enter, so a mention on a line of its own would submit the prompt in pieces. Images are never named in the text. The composed text must still fit the 8 KiB prompt limit. |
| Chat runs | Text files are mentioned as on a terminal run, and the composed text passes the same prompt check (the 8 KiB limit and the control-character rule). Each image is read when it is sent, through one handle opened without following a final link and without blocking on a FIFO: it must be a regular file, still at the canonical path its check found (the handle's final path on Windows, its device and inode on Unix), and it is read through the cap plus one byte at most. Each image becomes an image block, `{"type":"image","source":{"type":"base64","media_type":...,"data":...}}`, after the text block. The text block is left out when the text is empty. Only the Claude adapter carries images; another dialect refuses them with `agent.attachment_unsupported`. |
| Queue | A queued prompt is checked when it is queued, so a refusal stores nothing. Its names are stored in `agent_prompt_queue.attachments_json` (a migration, default `[]`). They are checked again when the prompt is typed, outside the service's status lock, since the check reads the disk; the head is read again under the lock and the check is used only when it is still the same prompt. A prompt whose files no longer check is failed with the refusal as its reason, which holds the prompts behind it; a send-now that finds them refused leaves the prompt waiting. |
| Not yet | The queue list does not show a queued prompt's attachments. |

## Verified and unverified vendor facts

Checked on 2026-11-26 in the public Anthropic documentation (platform.claude.com, vision; code.claude.com, Agent SDK
streaming input):

- Claude reads JPEG, PNG, GIF and WebP, as `image/jpeg`, `image/png`, `image/gif` and `image/webp`.
- The per-image limit is 10 MB base64-encoded on the Claude API, 5 MB on Amazon Bedrock. Maximum dimensions are 8000 x 8000 px.
- In the Agent SDK's streaming input, a user message is `{"type":"user","message":{"role":"user","content":[...]},"parent_tool_use_id":null}`,
  and its content may hold a text block and an image block with a base64 source.

Not verified, and only a real run can confirm them:

- That the installed Claude Code CLI accepts this stream-json user message on its `--input-format stream-json` input, for
  every version in the kind's chat range. The fake agent records the line but does not check it.
- That the CLI expands an `@path` mention in a stream-json user text block, and in typed terminal input.
- How the terminal UI treats a path with a space after `@`. Paths are not escaped or quoted: the documentation checked does not
  describe a quoted form such as `@"my notes.txt"`, so none is typed until a real run shows one works.

## Consequences

- A terminal run types the mention as text, so the transcript shows which file the agent was pointed at.
- A file replaced by a link after queuing is refused when its prompt is typed, because the names are checked again then.
- The rules are tested in three places: `crates/core-workspace` (containment), `crates/core-agents/src/attachments.rs`
  (caps, kinds, mentions) and `crates/cli/tests/agent_prompt_attachments.rs` (the public interface, with the fake agent).

## Checklist

- [ ] A link out of the workspace is refused before anything is typed (`a_link_that_leaves_the_workspace_is_refused`).
- [ ] An image on a terminal run is refused, and no text is typed (`an_image_is_refused_on_a_terminal_run_before_anything_is_typed`).
- [ ] A chat run sends the image block with the media type from its extension (`an_image_is_sent_as_an_image_block_on_a_chat_run`).
- [ ] The count, size and path refusals carry `agent.attachment_invalid` with the path in `data`.

## Next step

Show the attachments in the queue list, and check the unverified facts above against a real Claude Code run before the
capability is advertised to desktop users.
