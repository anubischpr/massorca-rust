//! Model picker: the one selector for agent, model, reasoning effort and speed in every
//! composer (agreements A18, A27 and A28). The trigger chip opens a three-column popover above
//! it: an agent rail, a searchable model list and the reasoning levels, with a Standard or Fast
//! footer. Access is the composer's own chip, never part of this picker.

pub mod state;

pub use state::{
    AgentAvailability, AgentOption, Choice, ModelList, ModelOption, PickerState, PickerView, RowRef,
};
