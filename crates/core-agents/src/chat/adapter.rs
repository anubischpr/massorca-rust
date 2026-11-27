//! The seam between the service and one vendor's structured dialect. The service owns the process, the journal and every
//! rule about items; an adapter only turns the dialect's lines into [`JournalOp`]s and the service's intents into bytes.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{
    ApprovalDecision, ApprovalScope, ChatItemBody, TokenUsage, TurnState,
};

/// A change an adapter asks of the journal for one line of output. Items are named by `key`s the adapter chooses (a
/// vendor's message or tool call id); the journal maps each to a stable item id for the rest of the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalOp {
    /// Adds an item to the open turn (or outside any turn when none is open). `parent` names the key of the sub-agent
    /// item whose run produced it. Adding a key the run already used replaces that item instead.
    Add {
        key: Option<String>,
        parent: Option<String>,
        body: ChatItemBody,
    },
    /// Replaces the body of the item `key` names; its position stays. Ignored for an unknown key.
    Replace { key: String, body: ChatItemBody },
    /// Appends text to the `assistant_message` or `thinking` item `key` names. Ignored for an unknown key or another
    /// item type.
    Append { key: String, text: String },
    /// Ends the open turn: its running messages complete, and the turn takes `state` and `outcome`. Ignored without an
    /// open turn.
    EndTurn {
        state: TurnState,
        outcome: Option<Outcome>,
    },
    /// The open turn's token usage as the CLI reports it (decision 0173). Ignored without an open turn.
    Usage { usage: TokenUsage },
    /// The CLI named its own conversation id (decision 0173): the session keeps it as its agent ref when it has none, so
    /// a resume finds the conversation. Written after the line's other ops.
    LearnRef { agent_ref: String },
    /// Bytes for the run's standard input, written after the line's other ops: the answer to a request the dialect
    /// requires one for and the adapter answers itself (a refusal of a permission request it cannot read). Never an
    /// authorization.
    Send { bytes: Vec<u8> },
}

/// The key under which an adapter adds the approval item of the permission request `request_id`, so the service finds
/// the item an `agent.chat.approval.answer` names (decision 0101, D6).
pub fn approval_key(request_id: &str) -> String {
    format!("approval:{request_id}")
}

/// A person's answer to a permission request, as the adapter encodes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalAnswer {
    pub decision: ApprovalDecision,
    pub scope: ApprovalScope,
    /// What a deny tells the agent.
    pub message: Option<String>,
}

/// One image a chat prompt sends as an image block (decision 0172): its media type and its bytes, base64 encoded. It
/// holds the person's file, so it prints its size and never its data.
#[derive(Clone, PartialEq, Eq)]
pub struct PromptImage {
    pub media_type: &'static str,
    pub data: String,
}

impl std::fmt::Debug for PromptImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PromptImage")
            .field("media_type", &self.media_type)
            .field("data", &format_args!("{} bytes", self.data.len()))
            .finish()
    }
}

/// One vendor's structured dialect. A run gets its own adapter, so an adapter may keep state between lines.
pub trait ChatAdapter: Send {
    /// The arguments the run starts with, from the arguments the kind's launch plan built (its declared arguments and the
    /// permission mode's, without a prompt: a chat's prompts go to standard input).
    fn launch_args(&mut self, planned: Vec<String>) -> Vec<String>;

    /// The bytes the run's standard input gets first, before any prompt: a dialect's handshake. None by default.
    fn opening(&mut self) -> Vec<u8> {
        Vec::new()
    }

    /// The bytes that send `text` as the person's next message, with their line ending. A dialect that cannot send it
    /// yet (its conversation is still opening) may hold it and return no bytes, then send it later with a
    /// [`JournalOp::Send`].
    fn encode_prompt(&mut self, text: &str) -> Vec<u8>;

    /// The bytes that send `text` and the `images` as the person's next message, with their line ending (decision 0172).
    /// `None` when the dialect cannot carry an image, which a prompt with images then reports as unsupported.
    fn encode_prompt_with_images(
        &mut self,
        _text: &str,
        _images: &[PromptImage],
    ) -> Option<Vec<u8>> {
        None
    }

    /// Whether the dialect can carry images: a prompt with images is refused before the journal takes it (decision 0172).
    fn carries_images(&self) -> bool {
        false
    }

    /// What one line of standard output (without its line ending) means for the journal. A line the adapter cannot read
    /// should become a `notice`, never an error.
    fn translate(&mut self, line: &str) -> Vec<JournalOp>;

    /// The bytes that answer the permission request `request_id`, or `None` when the request is not pending with the
    /// adapter (the CLI withdrew it, an interrupt cancelled it, or it was answered): then nothing may answer it. The
    /// adapter keeps the request until [`Self::forget_request`], which the service calls once the answer is recorded.
    fn encode_approval(&mut self, request_id: &str, answer: &ApprovalAnswer) -> Option<Vec<u8>>;

    /// Forgets the permission request `request_id`: it was answered.
    fn forget_request(&mut self, request_id: &str);

    /// The bytes that interrupt the running turn and keep the run; `None` when the dialect has no interrupt. Every
    /// request the adapter holds is forgotten: an interrupt cancels it, and nothing answers it afterwards.
    fn encode_interrupt(&mut self) -> Option<Vec<u8>>;
}
