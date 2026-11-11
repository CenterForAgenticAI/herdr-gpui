# Herdr 2 parity acceptance matrix

## Audit boundary and status rules

Baseline: **`389a55138`**, round 3, 2026-11-10. This is a source audit of
Sheprd's Herdr 1 and Herdr 2 paths, not a certification of full parity or of a
released build. Pending branches below are **not shipped evidence**. Update a
row only with the implementing commit, its PR, and the actual validation result.
The [session model](session-model.md) describes the ownership boundary: Herdr 1
owns workspaces/layout; Herdr 2 owns terminals and Sheprd keeps workspaces/layout.

- **Implemented**: a baseline code path exists for the stated scope. It does
  not mean native, phone, or remote runtime acceptance passed.
- **Partial**: some scope exists, but named hosts, lifecycle cases, or guarantees
  are missing. **Missing**: no baseline implementation for the stated scope.
- **Blocked**: an acceptance requirement needs a contract not currently available;
  client-side rereads do not close a daemon-side atomicity gap.
- **Pending**: validation has not run or has not supplied attributable evidence.
  A test file listed below is evidence of intended coverage, **not a test run**.

Paths below are repository-relative. `G` means `crates/sheprd-gpui/src`, `C`
means `crates/sheprd-core-client/src`, and `R` means
`crates/sheprd-remote-control/src`. Every feature row requires the applicable
cross-cutting acceptance checks in the QA ledger, as well as its row-specific
checks. No row can be accepted solely from a merged PR or a Greptile score.

## Owners and delivery evidence

Branches use `feat/h2r3-<owner>`; agents use `h2r3-<owner>`. A dash means no PR
has been reported to this ledger, not that no work is underway.

| Owner | Scope / dependencies | PR and implementation evidence | Actual validation |
| --- | --- | --- | --- |
| remote-worktrees | Host-aware Git work, scripts, cleanup; foundation for dispatch | —; baseline gaps confirmed by owner | Pending |
| dispatch | Destination discovery, eligibility and backend routing; consumes remote-worktrees | Draft [#1283](https://github.com/sheprd-labs/sheprd/pull/1283), reported `4d22e007f`; prepare `be29310fd`, remote-worktrees `af95a7887` included as `d4556cf10` | Owner reports 9 prepare and 4 required-core tests plus all-feature remote/vcs/usage check passed; setup fencing/feature closure, CI/review and native/cloud runtime pending. Provisioning spans providers/image layers |
| fanout | Multi-host lanes and durable recovery; consumes both foundations | Draft [#1281](https://github.com/sheprd-labs/sheprd/pull/1281), reported `f50b6016c`: ownership/receipt foundation; UI/journal integration still awaits dispatch selector | Owner reports 19 remote work tests passed; CI/review/runtime pending. No idempotent prompt-send contract after lost acknowledgement: replay suppressed with explicit uncertainty. Ambiguous checkout creation before receipt retained, never inferred owned |
| orchestrator | Repo discovery, remote dispatch and run controls; consumes foundations | —; owner confirmed snapshot-only readers and WSL identity loss | Pending |
| agent-sessions | Exact process-bound discovery and desktop/phone conversion | [#1285](https://github.com/sheprd-labs/sheprd/pull/1285), reported `3e2a87dd5`: typed Binding, explicit Codex/Copilot hooks, OpenCode TUI plugin/additive installer, owning-host discovery/conversion CLI and strict stale-label policy; exact dependency for consumers | Owner reports core 79 passed/5 ignored, client 7, ACP 9, JS 6 and all-target/all-feature clippy passed; CI/review pending. Pi/Omp/Letta, Windows reporters, remote OpenCode attribution, flag mapping and live QA remain partial; B01 separate |
| teleport-sessions | Session transfer and arrival; consumes agent-sessions | [#1278](https://github.com/sheprd-labs/sheprd/pull/1278), reported `7c0e4a457`: incremental hardening, shared discovery integration pending | Pending; remains partial: Pi/Omp/Letta producers, custom-root mapping, pre-copy source retirement/rollback, resume acknowledgement and durable arrival still outstanding; only atomic conditional stop is an upstream prerequisite |
| chat-restore | Desktop/phone restart recovery; conversion endpoint environment and stale-label suppression depend on agent-sessions | Draft [#1280](https://github.com/sheprd-labs/sheprd/pull/1280), reported `782da6f09`: recovery foundation, not full recovery | Owner reports 8 focused tests under default/all features and fmt/size/sorted/diff passed; CI/review/runtime pending. Automatic in-place launch blocked by B02; alternate new-terminal recovery still needs app-side durable layout remap and close tombstones |
| phone-prs | Standalone phone GitHub reads, sign-in, checks/logs | [#1282](https://github.com/sheprd-labs/sheprd/pull/1282), reported `614a1afe0`: explicit host-local `gh` opt-in, cached/paced/fenced badges/checks/logs and iOS capabilities/errors | Owner reports 11 targeted tests including standalone saved/owned API and badges, 61 changes tests, all-feature clippy and Swift capability test passed. CI/review/live phone/web/SSH/cloud/restart pending. Desktop cloud/WSL PR-origin lookup remains a separate gap |
| link-handlers | Explicit app-owned trusted per-host argv adapters | [#1279](https://github.com/sheprd-labs/sheprd/pull/1279), reported `c27c43e5e`; partial adapter support, not transparent H1 plugin compatibility | Owner reports fmt/sorted/file-size/diff passed; compilation/tests/CI/runtime pending, i18n baseline tracked by #1266. Compatible-action environment/remote quoting work continues. Manifest enable/platform registry and lifecycle/logs are app-owned work, not upstream blockers. H1 socket calls need plugin adaptation or a scoped compatibility service, never fake core IDs. Phone/web custom activation not demonstrated on H1 either: shared gap, not proven H2 regression |
| file-images | Bitmap paste, remote files/images including WSL | No PR reported; foundation `28b687794`, GPUI integration/tests in progress; upload/cleanup API coordinated with remote-chats | Pending; shared desktop QA must be coordinated. Windows native SSH/cloud upload retains H1 UploadUnsupported (shared platform limit), WSL maps without copying |
| remote-chats | Newly assigned: remote native chat launch, routing, splits, reconnect | —; baseline exclusion independently confirmed | Pending |
| remote-browser | Newly assigned: remote core browser reload, feedback and wait-return | —; owner confirmed shim reload refusal and discarded pane identity | Pending |
| phone-layout | Saved/own/external layout routes and stale desktop heartbeat audit | [#1284](https://github.com/sheprd-labs/sheprd/pull/1284), reported `91bc859bb`: lock authority, unclaimed exit withdrawal, active selection persistence/projection | Owner reports session 32, arrange 10, own 11, socket arrange 2 and web layout 7 tests compiled/passed. CI/review/native/direct paired-host QA pending. Desktop adoption still loses phone-owned grouping/names; saved-tab reopen preserves edits |
| desktop-prs | H2-only SSH kept-workspace PR origin/prefetch fencing | `feat/h2r3-desktop-prs`; no PR reported | Pending. Separate from daemon-local phone #1282; cloud/WSL origin absence is shared across generations |
| work-cli | Newly assigned: kept-workspace mute, scripts and fan-out CLI adapters; coordinates with domain owners | —; exact H1 guards verified during docs review | Pending |
| docs | This matrix, website baseline corrections, remote README, managed skill | [#1267](https://github.com/sheprd-labs/sheprd/pull/1267), `feat/h2r3-docs` | Documentation checks recorded in this PR; runtime checks pending |
| ci | Fluent checkout error, Windows cleanup fixtures and portable glob fixture | [#1266](https://github.com/sheprd-labs/sheprd/pull/1266), reported head `4c7a72daf`; conflicts resolved against main `387e650fc`/#1249, which landed overlapping `77521d729`; no patch consumed by this docs branch | Owner preserved main's KEY/portable fixture, leaving six-file GPUI residual changes; reports quick checks including 5962 Fluent messages passed. Prior-head macOS/Linux results and `276bae9bf` 5/5 are not current-head approval; latest CI/review pending |

Feature owners keep their implementation docs with their feature PRs. Docs owns
this matrix and baseline corrections in `website/docs/{chat,terminals,worktrees,
teleport,orchestrator}.md`, the remote-control README, and managed skill guidance.
The orchestrator/fanout/worktree owners use their GUI README sections; worktrees
also owns session-model updates. Chat restore uses `docs/rich-chat.md` and the ACP
README; Teleport and handlers use dedicated feature audit docs. Reconcile those
at landing rather than copying unmerged behavior into these baseline claims.

## Desktop, hosts and terminal coverage

| ID | H1 reference capability / H2 scope | Baseline H2 status and source evidence | Responsible owner | Required acceptance / actual result |
| --- | --- | --- | --- | --- |
| D01 | Start without H1; coexist without migration | **Implemented** local fallback and pinned core install: `G/core_terminals/fallback.rs`, `install.rs`, `hosts.rs`; `scripts/release/herdr-core.json` | coordinator; CI packaging | Q01, Q02, Q07 pending |
| D02 | Terminal input, modes, selection, IME, copy, find, history, clear, editor, mirrors | **Implemented** core view modules `G/core_terminals/view/{keys,pointer,paste,history,find,copy_mode}.rs`; tests under `G/core_terminals/tests/` | coordinator | Q01–Q03 pending; tests located, not run by docs |
| D03 | Splits, zoom, resize, move, close, saved layout and multi-window | **Implemented** `G/core_terminals/{split,moves,layout,tabs}.rs`; `tests/{moves,restore,zoom_drawn,shared_names}.rs` | coordinator | Q02–Q04 pending; verify observer cannot steal input and closing app preserves terminals |
| D04 | SSH/cloud/WSL core transport and host setup | **Partial** transport/install exist (`G/core_terminals/{hosts,install}.rs`), but downstream feature rows retain local-only gates; Windows client SSH restrictions remain | dispatch / remote-worktrees | Q01, Q02, Q07 pending per platform/provider; never treat transport connection as all-feature support |
| D05 | Status, sidebar, attention, notification/mute, checkpoint observation | **Implemented** shared sessions plus `G/core_terminals/{session,attention,mutes}.rs`; `crates/sheprd-vcs/src/checkpoint`; remote Git identity remains partial | coordinator / remote-worktrees | Q01–Q04 pending; compare daemon status, no client-local guessed completion |
| D06 | Inline kitty images | **Partial** `crates/sheprd-terminal/src` and `G/core_terminals/tests/images.rs`: in-band images supported, file/shared-memory graphics not loaded; historical image replay not promised | file-images / coordinator | Q03 pending; distinguish rendering from bitmap paste; bounded malformed payload tests and restart behavior |
| D07 | Clipboard bitmap paste and live screenshot thumbnail | **Missing** terminal bitmap bridge: `G/core_terminals/view/paste.rs::paste_clipboard` handles paths/text only; thumbnail path can be unreadable | file-images | Q03 pending for macOS, Linux X11/Wayland, Windows; no implicit Enter |
| D08 | File drops/copies on SSH/cloud/WSL | **Partial** local escaped paths work; `paste_paths` rejects remote with `OtherHost`. H1 has SSH upload and WSL translation (`G/window/{file_drop,wsl_drop}.rs`); cloud upload is not a universal H1 capability | file-images | Q03, Q04 pending: destination paths, size/count bounds, cancellation/reconnect, wrong-pane refusal |
| D09 | Terminal links and custom plugin handlers | **Partial** built-in links work; `G/window/links.rs::open_core_link` bypasses H1 handlers; core view discards raw OSC8 and remote file targets | link-handlers | Q03, Q07 pending: ordered first-match, explicit click only, host-correct argv, failure/fallback, arbitrary schemes |
| D10 | Settings, themes, shortcuts, shared UI settings | **Implemented** app settings plus `G/core_terminals/tests/settings.rs`; H1 daemon-only settings such as scrollback restoration are not core settings | coordinator | Q03, Q07 pending: live apply, restart, keymaps, copy-on-select, scroll amount, right-click modifier, narrow UI |
| D11 | Agent notes from terminal selection | **Implemented** `G/window/annotate/core.rs`, `tests/core_notes.rs`: core reply, clipboard on gone/refused/unreachable | coordinator | Q03/Q04 pending; test inventory inspected, not executed; website's “no notes yet” corrected |
| D12 | Browser/review feedback and remote CLI shim | **Partial** local core targeting exists; remote core shim supports browser open via labels, not reload/return notes (`skills/herdr-gpui-browser/SKILL.md`, `G/core_terminals/requests.rs`) | remote-browser | Q03/Q06 pending: origin identity, no delivery to another host's same terminal ID, report unsupported reload/feedback |

## Work, agents and automation

| ID | H1 reference capability / H2 scope | Baseline H2 status and source evidence | Responsible owner | Required acceptance / actual result |
| --- | --- | --- | --- | --- |
| W01 | Workspaces and Git worktree create/open/list/delete | **Partial** local kept work supported; `G/menu/workspace/kept_worktrees.rs` rejects remote; `G/core_terminals/session.rs` omits remote Git metadata | remote-worktrees | Q01/Q04/Q06 pending: host-scoped Git identity, existing branch, failure cleanup, no wrong local checkout |
| W02 | Setup/run/archive and merged cleanup | **Partial** local `G/worktree_scripts/kept.rs`, `G/merged_cleanup/kept.rs`; config reader rejects WSL/cloud, archive lookup and landing local-only | remote-worktrees | Q04/Q06 pending: exact-content trust, script exit, cancel, dirty/unpushed/merged-head checks, keep branch |
| W03 | Smart Dispatch, ranking, repo discovery/cloning, provisioning | **Partial** `G/dispatch/hosts.rs::dispatch_keeps` local-only; remote destinations depend on H1 snapshots | dispatch | Q01/Q04/Q07 pending: H2-only, mixed-generation, load, missing repo, provider setup, no silent H1 fallback |
| W04 | Fan-out spread, compare/keep/discard, recovery | **Partial** `G/fan_out/{state,ui,job}.rs`, `G/menu/fan_out.rs` constrain kept lanes to local; owner reports ambiguous Prompting replay and lost terminal/destination after crash | fanout | Q04/Q06 pending: durable owned IDs before side effects, exactly-once prompt or explicit ambiguity, remove only owned lanes |
| W05 | Orchestrator repo listing, dispatch, open/focus, reply/stop/restart/remove | **Partial** `G/orchestrator/host.rs` H1-only repo/live-agent readers; `view/dispatch.rs` remote H1; `view/core_run.rs` and `core_removal.rs` reject remote | orchestrator | Q01/Q04/Q06 pending: remote repo/run placement, WSL identity, prompt/model, durable controls, no unrelated cleanup |
| W06 | Local native chat launch, attach, multiple chats, transcript | **Implemented** `G/chat/core_pane.rs`, core labels and local chat tests; app restart reattachment is not program-loss recovery | chat-restore | Q02/Q03/Q05 pending; reconnect to still-running ACP separately from reboot/lost process |
| W07 | Remote native H2 chat launch/attach/split/reconnect | **Missing** `G/core_terminals/tests/chats/remote.rs` explicitly asserts exclusion; `G/chat/core_pane.rs` uses `Route::Local` | remote-chats | Q01–Q05 pending: host-bound endpoints, remote companion install/version, same t1 on two hosts, attachments and desktop-open/closed |
| W08 | Terminal-to-chat conversion: Claude, Codex, Copilot, OpenCode | **Partial** `C/convert.rs::session` accepts only Claude; desktop and phone share it. Gemini/Pi are not supported H1 conversion targets either | agent-sessions | Q02/Q04/Q05 pending: exact session/process, flags/custom dirs, integration installed/missing, no newest-session guesses; B01 blocks atomic handoff |
| W09 | Teleport H1/H2 all source/destination combinations, back, remote-only workspaces | **Partial** `crates/sheprd-remote/src/teleport/core/sessions.rs` Claude discovery only; local kept workspaces supported; remote kept workspaces not offered | teleport-sessions / remote-worktrees / dispatch | Q01/Q04 pending in all four generation combinations; session files/config roots, rollback, cancellation, remote-only repo |
| W10 | Teleport continuity and durable arrival | **Partial** owner source audit: `crates/sheprd-remote/src/teleport/job.rs` retires before transfer, can restart fresh after transfer failure, treats queued launch as arrival; cancellation can drop destination after retirement | teleport-sessions | Q04 pending: session acknowledgement before retirement, recover ambiguous arrival, never call handoff continuity; B01 applies |
| W11 | Chat process-loss/core-reboot recovery without duplicate agents | **Missing** H2 recovery: `G/chat/restore.rs` H1 snapshots/CLI; ACP restore keeper requires H1 target; `R/hub/core/create.rs` chat `keep:false` | chat-restore | Q02/Q05 pending: explicit closure vs loss, saved cwd/env, claims, transcript continuity; B02 applies |
| W12 | Automation schedule/manual runs, worktree and agent steps | **Implemented** H2 driver `crates/sheprd-automate/src/herdr_core.rs`; automation executes on its host, not through desktop worktree menu | coordinator | Q01/Q02/Q04/Q07 pending: no H1 installed, approval hash, schedule restart, script failure, named core, installed core/agent prerequisites |
| W13 | Mods state and actions / legacy plugins | **Partial** shared `Session` exposes core agents/workspaces to mods; legacy H1 plugins may require H1 pane IDs/API socket and cannot be presumed portable | coordinator / link-handlers | Q03/Q06 pending: core identity in state/effects, explicit unavailable actions; inventory plugin contracts individually |
| W14 | Notes, checkpoint restore, Teleport, mutes, scripts and fan-out CLI | **Partial** local notes/restore/Teleport target kept workspaces; `G/control/server/work/mute.rs` and `work/flows.rs::launch` call `found.herdr`, and `flows.rs::fan_out` calls `needs_herdr`: mutes/scripts/fan-out still exit 8 without H1 workspace state even where desktop menus work. Host gaps W01/W02 also apply | work-cli (adapters), remote-worktrees/fanout (domain implementations); docs corrects guidance | Q06 pending: exercise each actual CLI handler, not only menu routing; caller scope, unsupported exit codes and confirmations |
| W15 | Desktop PR lookup/prefetch for SSH kept workspaces | **Partial** `G/menu/pr.rs:217–220` requires a connected H1 snapshot for remote prefetch; kept-only exception is Local. `G/menu/pr/core.rs:18,28` fallback uses main tokens/Local. Cloud/WSL absence in `endpoint_pr_origin` and `sheprd-github/src/pull_request/{model,fetch}.rs` is shared H1/H2 limitation, not this regression | desktop-prs | Q01/Q04/Q06 pending: host-correct token/origin, no H1 snapshot, two hosts with same IDs, stale response fences. Daemon-local P06 is separate |

Teleport's H1 transferable agent set is Claude, Codex, OpenCode, Pi, Omp,
Copilot and Letta (`AgentKind::carries` in `sheprd-client`). Other H1 kinds
already use handoff; do not count that as a new H2 regression. Conversely,
supporting only the four chat-convertible agents is insufficient for Teleport.

## Phone and web coverage

| ID | Capability | Baseline status and evidence | Responsible owner | Required acceptance / actual result |
| --- | --- | --- | --- | --- |
| P01 | Pair/authenticate/revoke; direct SSH/cloud pairing; push | **Implemented** `R/auth`, `R/control`, `R/push/terminals.rs`; direct host enrollment is distinct from desktop relaying | coordinator | Q05/Q07 pending: iOS and web, online/offline, revoke on unreachable host, desktop closed, no credential exposure |
| P02 | Home/repo/branch listing, saved tabs and external core document | **Implemented** `R/hub/core/{projection,saved,spaces,checkouts}.rs`; saved tabs retain membership, malformed document falls back | coordinator | Q02/Q05 pending: empty core, named H1 coexistence, labels and duplicate IDs, bounds/malformed document |
| P03 | Terminal screens/history/input/resize/close | **Implemented** `R/hub/core/{screen,hold,create}.rs`; shared bounded attachments and observer/driver transitions | coordinator | Q03/Q05 pending: two phones, desktop open/closed, observer dimensions, detach/expiry and loss, no input replay |
| P04 | Phone split/zoom/rename while desktop closed | **Partial** saved/own fallback exists: `R/native/changes.rs::arrange` routes to `R/hub/core/arrange.rs` when desktop heartbeat expires; `R/web/terminal.rs` uses this too and workspace rename has its own fallback. Baseline heartbeat/active-pane gaps addressed in pending #1284. Remaining client gap: `G/core_terminals/adopt.rs:103` creates `Panes::single` for owned terminals; `own::Store::fit` removes adopted members, losing phone grouping/names. External `herdr-app` tabs are intentionally refused. Initial blanket missing claim was disproved | phone-layout | Q02/Q05 pending: stale heartbeat, saved-file concurrency, active pane, own-layout adoption, daemon restart, native/web routes and external-layout refusal; saved-tab reopen preserves edits |
| P05 | Create shell/chat/workspace/worktree without desktop | **Implemented** `R/hub/core/create.rs::open_worktree_chat` adds Git checkout, labels chat; ambiguous start retains checkout | remote-worktrees / chat-restore | Q04/Q05 pending: branch collision, no-home/invalid cwd, failed versus ambiguous spawn, adopt into desktop without duplicates |
| P06 | Standalone Git diff vs standalone GitHub PR/check/log | **Partial** Git fallback implemented in `R/native/changes/local.rs`; Checks returns empty default and CheckLog returns NoLog; PR badge cache depends on desktop | phone-prs | Q05 pending: no desktop cache, explicit auth, expiry/quota/offline errors, paired SSH/cloud, iOS capability display |
| P07 | Convert terminal agents, recover chat after loss | **Partial** phone conversion shares `C/convert.rs`; lost chats finish (`keep:false`) | agent-sessions / chat-restore | Q02/Q05 pending; B01/B02; native and web must report refusals honestly |
| P08 | Update machine from phone | **Partial** `R/control/app_update.rs` relays to a recent desktop process report; cannot update with app closed | coordinator | Q05/Q07 pending: stale report/new process refusal; distinguish desktop updater from remote host service setup |

## Exact upstream prerequisites

**B01 — atomic process-bound stop/send (blocked).** The baseline
`C/convert.rs::run` reads a terminal, then sends `core.keys(..., ["ctrl+c"])`,
then eventually `core.send`. These are separate requests. The pinned core's
`terminal.keys` has no expected foreground PID/status guard (`C/connection.rs::keys`
serializes only terminal and keys); `terminal.send` (`C/wire.rs::SendText`)
has a status guard, not a process identity guard (agent-sessions and Teleport
owners independently reported this contract). A shell/agent can change after
a client reread and before mutation. Required upstream contract: condition the
mutation atomically on terminal incarnation and foreground process identity
(including a reuse-resistant generation/start identity), plus expected status;
refuse without input on mismatch. Session discovery hooks can prove which
session a process reported, **not** that the process still owns input when
Ctrl+C or a launch line lands. A client-only reread is not acceptance evidence.
Track conversion and Teleport retirement separately until upstream tests prove
the conditional operation. External Herdr/core checkouts remain read-only.

Agent-sessions additionally inspected the installed `herdr-core schema`:
`terminal.keys` parameters are only caller/keys/terminal and have no
`status_mismatch` error; `terminal.send` accepts only an IdleOrWorking `if`
condition and may wait roughly three seconds for quiet before typing. Foreground
records have pid/name/argv/cwd, no process-birth identity; Status has no session.
The acceptance failures to test are a work/selection change after the final read
but before interrupt, and a replacement foreground program before send (including
that quiet interval). Preserve the schema/version artifact with any upstream fix.

**B02 — conditional chat restart (blocked guarantee).** `terminal.send` cannot
atomically fence process identity, cwd and recovery generation; no
`terminal.restart` contract is available in the pinned API according to the
chat-restore audit. Need a daemon-side conditional restart/claim or equivalent
atomic launch contract to prove a recovered idle shell cannot change ownership
before a resume command. Persistence and distinguishing explicit close from
loss are implementable app work; they do not alone establish safe exactly-once
restart. Record exact upstream version/PR and race tests when available.

**Integration prerequisites (implementable, not B01).** Proposed app-owned
Codex/Copilot SessionStart hooks and OpenCode TUI route plugin must be installed
on the agent's actual host/config root and bind terminal/process/session with
explicit Sheprd provenance. Acceptance needs missing/stale/replaced bindings,
custom configuration directories, process reuse, remote host collisions and
agent restart. Do not reuse H1 integration installation as proof or guess the
latest session. Installation UX/CLI and documentation belong to agent-sessions.

## Runtime and CI QA ledger

Every feature owner must attach command/scenario, implementing head SHA, target
platform/host, expected and observed result, and artifact/log or test name to
the relevant IDs. “CI green” without the job/head is insufficient. All runtime
checks below are **pending** in this documentation audit; no desktop was driven.

| ID | Required scenario set | Rows | Result / accountable owner |
| --- | --- | --- | --- |
| Q01 | H2 only with H1 absent; H1 installed but stopped; both running; explicit socket/session; local, saved SSH, each supported cloud provider, WSL; verify no silent H1 fallback | D01–D05, W01–W09, W12 | Pending; each feature owner, coordinator assembles host coverage |
| Q02 | App quit/reopen, core restart, ACP loss, network reconnect, deliberate close, concurrent windows/phone; retain identity and no duplicate/replayed input | D01–D05, W06–W08, W11–W12, P02/P07 | Pending; distinguish process survival from resume |
| Q03 | Native narrow/long-label UI, focus/IME/popups, select/copy/scroll, image/file bounds, correct pane/host; light/dark when visual behavior changes | D02–D12, W06/W07/W13, P03 | Pending; coordinate shared desktop; headless layout is not native QA |
| Q04 | Cancel before/after each side effect; disconnect/timeout ambiguity; restart recovery; ownership-only cleanup; durable IDs and source retention until acknowledged arrival | D03/D05/D08/D11, W01–W05/W08–W10/W12, P05 | Pending; deterministic tests plus remote runtime |
| Q05 | iOS and web separately, desktop open/closed, local and directly paired SSH/cloud; permissions, refusal display, stale/offline, two clients, auth/cache bounds | W06/W07/W11, P01–P08 | Pending; neither desktop nor Rust tests certify iOS |
| Q06 | CLI help/parse before GUI, `--json`, `--window`, exit codes, correct caller scope, unsupported capability and existing confirmation dialogs | D12, W01/W02/W04/W05/W13/W14 | Pending; relevant executable CLI suites in CI |
| Q07 | macOS/Linux/Windows default and all-feature build/tests; platform-only cfg; service install/upgrade, bundled core/version, dependency presence | D01/D04/D09/D10, W03/W12, P01/P08 | Pending; CI owner #1266 addresses baseline failures; feature owners fix their regressions |
| Q08 | Full PR diff, exact head CI checks green, latest meaningful head Greptile 5/5, zero unresolved findings; no merge | All delivery rows | Pending per PR; review score is not feature/runtime acceptance |

Use repository recipes only with isolated explicit binaries/resources:
`just test-core /absolute/path/to/herdr-core`,
`just test-live /absolute/path/to/herdr`, `just test-gui /absolute/path/to/herdr`,
`just test-sidebar`, `just test-perf`. They are **not run by this docs audit**.
Do not stop a personal daemon or use the shared desktop unannounced. Long builds,
default/all-feature workspace suites and platform gates go to CI. Documentation
changes require path/command/link review and `git diff --check`; their results
belong in the docs PR QA checklist.

Documentation validation on this branch: `git diff --check` passed;
`website/tools/docs.py --check` passed (26 pages); `python -m unittest discover
-s website/tools -p 'test_docs.py'` passed (11 tests). The last two used an
isolated virtualenv with `pip install --require-hashes -r website/requirements.txt`
after the system Python correctly failed with missing `markdown`. No Rust,
native, remote, or iOS runtime suite was run by this audit. CI foundation #1266
was queried at `276bae9bf`: open, Workflow Security passed, Commit messages
queued; this is not a green full-CI result.

## Remaining recommendations and acceptance gate

1. D12 and P04 now have remote-browser and phone-layout owners: finish remote
   browser feedback/reload and verify existing desktop-closed layout routing.
   The stale README refusal is not evidence that all standalone layout is absent.
2. Keep D07/D08 and W07 independent of worktree parity: new owners file-images
   and remote-chats were assigned after source evidence was sent to coordinator.
3. Publish a per-capability unavailable reason to iOS/web and CLI; an empty
   success (notably P06 checks) must not imply the feature was verified.
4. Inventory legacy plugin contracts and all supported Teleport agents before
   calling explicit app-owned handler configuration “plugin parity.”
5. Record native/remote/iOS artifacts against the same commit that is proposed
   for delivery. An unrun case stays pending even after merge or 5/5 review.
6. Reaudit baseline rows when foundations land; preserve outstanding failures
   and upstream blockers rather than replacing the matrix with a completion
   percentage. **100% coverage is not currently established.**
