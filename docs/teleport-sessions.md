# Teleport session continuity audit

This is an implementation acceptance record, not a claim that every agent
resumes across hosts. A handoff starts a new conversation; it is not session
continuity.

## Backend combinations

`sheprd-remote/src/teleport/job/plan.rs` selects the destination generation
independently of how the source session was discovered. `sessions.rs` carries
the native session storage; `job.rs` launches into either generation.

| Source → destination | Session identity today | Acceptance still required |
| --- | --- | --- |
| H1 → H1 | H1 integration reference, matched to the detected agent | Live transfer, exact destination session acknowledgement |
| H1 → H2 | Same H1 reference and transfer implementation | Live transfer, durable app-kept destination, exact session acknowledgement |
| H2 → H1 | Claude PID/start record; installed Sheprd reporters through the owning-host helper | Remaining agent producers, live transfer |
| H2 → H2 | Same source discovery as H2 → H1 | Remaining agent producers, durable app-kept destination, live transfer |

Before a move and again immediately before retirement, resumable sessions of
both generations are re-read. Session reference (including provenance), retained
flags, and working directory must match the review. These are fresh checks,
not an atomic ownership guarantee: another client can change the process or
conversation between checking and stopping it.

Non-Claude discovery consumes `sheprd-client::herdr::sessions::Binding` from
the owning host's `sheprd-acp session-discover TERMINAL AGENT`, using the same
socket/session environment as Teleport's core commands. The complete binding
(core, terminal, PID, birth token, selection sequence, provenance and config
metadata) is retained in the review and compared again before retirement.
Explicitly absent or cleared selections permit handoff; malformed, stale or
failed reads refuse the move. Missing helpers permit handoff, not guessed
session IDs. Claude's native record is attributed to Sheprd's own validation,
not impersonated as a Herdr 1 integration event.

This integration depends on [#1285](https://github.com/sheprd-labs/sheprd/pull/1285),
initial foundation `3e2a87dd5`. Its initial producers cover Codex/Copilot
SessionStart hooks and the OpenCode V2 TUI plugin; installing the helper alone
does not install those reporters. See [session evidence](agent-session-evidence.md).

## Agent coverage

The existing transferable subset is `AgentKind::carries` in
`sheprd-client/src/herdr/agent_kind.rs`:

| Agent | Transfer mechanism | Remaining evidence needed |
| --- | --- | --- |
| Claude | Project transcript and subagent archive | Foreground-specific config root, live round trip |
| Codex | Exact rollout filename suffix, preserving dated path | Installed SessionStart reporter, custom config root mapping, live round trip |
| OpenCode | Native export/import | Installed TUI reporter, exact import/resume acknowledgement, live round trip |
| Pi / Omp | Exact session path or full filename ID suffix | Process-bound discovery, custom session roots, live round trip |
| Copilot | Whole session-state directory | Installed SessionStart reporter, custom config root mapping, live round trip |
| Letta | Server-side conversation reference | Process-bound discovery and destination server/account identity |

Duplicate Claude, Codex, Pi, or Omp files for one reference are refused rather
than choosing the first file. Pi/Omp ID lookup is limited to the session tree
and matches the whole ID suffix, not a substring. Transfer rejects references
whose agent, reference kind, or provenance is not accepted for that agent.

Other `AgentKind` variants already lack confirmed cross-host storage support
for H1. Preserving H1's *same-host* resume commands does not establish that
their history is available on a different host. Each requires a documented
storage/export contract before being added to the transferable subset.

## Host and return paths

- Local, saved SSH and cloud hosts execute discovery and transfer on the
  selected host. A disconnected app-kept destination probes Git directly;
  it does not query a nonexistent H1 inventory.
- Teleport back uses `Destination::Reclaim`, checks repository identity, backs
  up the checkout, and chooses the keeper of the original workspace. Session
  transfer is the same in both directions; it is not a special resume shortcut.
- WSL and custom socket hosts are explicitly rejected by `Host::new`.
  Native Windows Teleport is not implemented. These are not covered by macOS
  fixture tests.
- Non-Claude bindings with nonempty source configuration overrides are explicitly
  refused until a destination mapping exists. Absolute source roots are never
  applied on the destination, or silently discarded to read a different store.
  Legacy H1 and native Claude transfer scripts still use the host shell
  environment; foreground-only configuration overrides remain to be handled.

## Unresolved lifecycle gaps

1. `job::run` retires source programs before reading/copying their session files.
   Copy failure currently produces `AgentStartedAfresh`. Cancellation or launch
   failure after retirement can leave no running source conversation. This is
   not transactional rollback and remains a completion blocker.
2. `core::type_line` and H1 `pane run` acknowledge submitting input, not resuming
   the expected conversation. Source retirement needs an exact destination
   session acknowledgement, with a bounded wait and explicit failure.
3. `teleport/ui/unclaimed.rs` retains successful outcomes only in an in-memory
   queue until `settle_teleport` saves app tabs. Closing one window is handled;
   process loss before persistence is not durable arrival. Destination identity
   and recovery state must be persisted before source retirement.
4. There is no conditional PID/start/session precondition on the core's stop
   operation. Fresh checks cannot make stopping atomic. A real upstream contract
   is required; no external Herdr checkout is changed by this work.
5. Shared discovery's remaining Pi/Omp/Letta producers are owned by the round-3
   `agent-sessions` slice. Without explicit Sheprd evidence those sessions still
   hand off. Startup argv, the newest transcript, and fabricated H1 provenance
   are not valid substitutes.

## Validation

Deterministic tests cover mismatched agent/provenance, changed flags/cwd,
duplicate session files, complete Pi ID matching, cancelled server-backed
references, and daemon-free destination discovery. Existing script fixtures
cover Claude, Codex, Copilot and Pi transfer formats. These fixtures do not
prove a real agent resumed the same conversation.

Pending: live H1/H2 four-way transfers and return trips; multiple concurrent
agents with different roots; source replacement during transfer; cancellation
at every phase; destination restart or app crash before arrival persistence;
SSH/cloud execution; platform CI; native UI checks. Overall parity remains
incomplete until the dependencies and lifecycle gaps above are closed.
