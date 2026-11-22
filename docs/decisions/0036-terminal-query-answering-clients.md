# 0036: Terminal query-answering clients

Status: accepted, 2026-11-21.

## Context

A program that sends a cursor-position query (`ESC [ 6 n`) waits for the
answer, and ConPTY renders nothing until it has one. 0034 holds the query while
a client is attached, so the attached client's emulator can answer it. The
service answers a held query itself only when another client attaches or when
every client that received it has detached. A client that stays attached and
never answers, such as a log tail or a script that reads the output, therefore
stalls the shell for as long as it stays attached. The service cannot tell such
a client from an emulator that is slow to reply, so it has to wait on both.

## Decision

- **A client declares it at attach.** `terminal.attach` takes an optional
  `answers_queries` flag. `true` means the client is a terminal emulator that
  answers the cursor queries in the output it reads. Absent or `false` means it
  does not, and the service answers for the terminal. An old client that sends
  nothing is not an emulator.
- **Only declared emulators hold a query.** A query that arrives while a
  declared emulator is attached is held for the emulators attached at that
  moment, as in 0034. A query that arrives while no declared emulator is
  attached, whether a log tail is attached or nobody is, is answered at once,
  as with no client attached.
- **The rest of 0034 holds.** An attach answers every held query. When the last
  emulator that received a held query detaches, the service answers it if
  needed and drops it. A late reply to an answered query is dropped. Input from
  any client is handled as before.
- **The callers.** The interactive `terminal attach` declares `true`: the user's
  terminal answers the program's queries, and the CLI forwards those replies on
  stdin. The desktop feed and the test clients that answer queries declare
  `true`. `terminal screen`, which reads one snapshot, and any reader that does
  not answer declare nothing. In the core, the declaration is
  `QueryAnswers::Client` or `QueryAnswers::Service`.

## Alternatives

- **A timer.** Answer a held query after a delay. Rejected: the answer then
  races the emulator's own reply, which can still arrive. The shell reads two
  answers, or the emulator's real answer is dropped, and the right delay
  depends on the machine.
- **Infer the emulator from its behaviour**, for example from a reply it has
  sent before. Rejected: a client has not replied before its first query, so
  the service would answer that query itself, and the client's real reply would
  then reach the shell as typed input. Inference would also need per-client
  history.
- **Treat old clients as emulators.** Rejected: an old client that never answers
  would keep stalling the shell, which is the bug this decision removes.
- **Always answer from the service's model.** Rejected: 0034 keeps the attached
  emulator's answer instead (TERM-15).

## Consequences

- **Wire.** The flag is an optional addition inside `terminals/1`, which 0035
  allows. An old service ignores it, so a new client on an old service behaves
  as before: every attached client holds queries. A new service reads a client
  that sends nothing as not answering.
- **Mixed versions.** An old emulator client on a new service does not declare,
  so the service answers its queries, and the emulator's own reply reaches the
  shell as typed input. Upgrade clients before the service they attach to.
- A client that answers without declaring is not waited for. Its replies are
  handled as input today: each reply takes the oldest held query, which may
  belong to an emulator, or reaches the shell when no query is held.
- A declared emulator that never replies still stalls the shell while it stays
  attached, until another client attaches or it detaches. That is intended: an
  emulator is expected to answer.
- No timers, and no state beyond one flag per attachment.
