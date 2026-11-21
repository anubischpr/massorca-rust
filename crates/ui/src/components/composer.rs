//! The composer: the box where the user writes to an agent (Composer.md, agreements A29,
//! A32 and A33). It is the approved session body: attachment chips, the text area, and a
//! bottom row with attach, access, the model picker, the microphone and the primary button,
//! which sends, queues or stops. The rules are in [`policy`]; the owner receives what the user
//! asked for as [`ComposerEvent`]s.

pub mod policy;
