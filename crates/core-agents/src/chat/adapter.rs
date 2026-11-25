//! The seam between the service and one vendor's structured dialect. The service owns the process, the journal and every
//! rule about items; an adapter only turns the dialect's lines into [`JournalOp`]s and the service's intents into bytes.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::chat::{ApprovalDecision, ApprovalScope, ChatItemBody, TurnState};

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
}

/// A person's answer to a permission request, as the adapter encodes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalAnswer {
    pub decision: ApprovalDecision,
    pub scope: ApprovalScope,
    /// What a deny tells the agent.
    pub message: Option<String>,
}

/// One vendor's structured dialect. A run gets its own adapter, so an adapter may keep state between lines.
pub trait ChatAdapter: Send {
    /// The arguments the run starts with, from the arguments the kind's launch plan built (its declared arguments and the
    /// permission mode's, without a prompt: a chat's prompts go to standard input).
    fn launch_args(&self, planned: Vec<String>) -> Vec<String>;

    /// The bytes that send `text` as the person's next message, with their line ending.
    fn encode_prompt(&mut self, text: &str) -> Vec<u8>;

    /// What one line of standard output (without its line ending) means for the journal. A line the adapter cannot read
    /// should become a `notice`, never an error.
    fn translate(&mut self, line: &str) -> Vec<JournalOp>;

    /// The bytes that answer the permission request `request_id`.
    fn encode_approval(&mut self, request_id: &str, answer: &ApprovalAnswer) -> Vec<u8>;

    /// The bytes that interrupt the running turn and keep the run; `None` when the dialect has no interrupt.
    fn encode_interrupt(&mut self) -> Option<Vec<u8>>;
}
