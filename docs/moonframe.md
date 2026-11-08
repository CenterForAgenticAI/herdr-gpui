# Sheprd on Moonframe

`just run-moonframe` starts Sheprd in a window [Moonframe](https://moonframe.dev)
lays out. It is early and separate from the app `just run` starts: quit Sheprd
first, as both use your settings and daemon.

Everything around the terminals is Luau in `crates/sheprd-moonframe/app`:

| File | Draws |
| --- | --- |
| `main.luau` | The layout: where each part goes |
| `mods/header/main.luau` | Back, Forward, and the sidebar toggle |
| `mods/activity/main.luau` | The activity bar |
| `mods/workspaces/main.luau` | Worktree rows, grouped by repository, and agents |
| `mods/details/main.luau` | The right pane; open it from Details in the status bar |
| `mods/status/main.luau` | The status bar |

The terminals, tabs, and pages stay one Rust view, placed with
`ui.view { name = "sheprd", id = "main", props = { region = "main" } }`.

## Changing it

Saving a file reloads the window. To keep a change of your own, copy the file
to the same path under `~/.config/sheprd-moonframe` and edit the copy; deleting
it brings the original back. The sparkles at the bottom right ask an agent to
make a change, and it asks before each edit. View > Inspect Element (⌥⌘I, or
⇧⌘X to go straight to picking) shows what built any part, and its file and line.

Run `cargo run -p sheprd-moonframe -- --check` after a change. It loads what is
drawn at that moment, so a mistake in a row that only appears with an agent
shows in the log when one appears: `unknown key` names it.

## `app.sheprd`

| Field | Holds |
| --- | --- |
| `workspaces` | In the sidebar's order: `id`, `number`, `label`, `branch`, `repo`, `repo_key`, `linked`, `child` (shown under its checkout), `focused`, `agent_status`, `ahead`, `behind`, `added`, `removed`, `dirty`, `pr` |
| `agents` | `name`, `status`, `title`, `pane`, `workspace` |
| `panels`, `panel` | The activity panels (`id`, `label`, `count`) and the current one's `id` |
| `connected`, `status` | Whether the daemon answers, and what to say about it |
| `section`, `sidebar_visible`, `status_bar_visible` | What the window shows |

Two parts change on their own, so they have their own providers and a change
runs only the views that read them:

| Provider | Holds |
| --- | --- |
| `app.sheprd_load` | `cpu` and `memory`, as percentages |
| `app.sheprd_theme` | `accent`, `active`, `background`, `danger`, `foreground`, `muted`, `sidebar`, `subtext`, `success`, `surface`, `warning` |

Moonframe's own widgets follow Sheprd's theme and UI font without reading these.
`font = "mono"` is Sheprd's sidebar font.

## Asking Sheprd for something

Send a message with `to = "provider:sheprd"`:

| Message | Does |
| --- | --- |
| `{ command = "name" }` | Runs any command by its name in the catalog, such as `new_workspace`, `toggle_sidebar`, or `themes` |
| `{ focus_workspace = id }` | Focuses a workspace |
| `{ focus_pane = id }` | Focuses a pane |
| `{ panel = id }` | Shows an activity panel |

## What is not there yet

The window does not yet answer the `sheprd` command line, send system
notifications, open links, update itself, show Sheprd's menus, remember its
position, or open a second window. The rows have no context menus, dragging, or
search, and the sidebar's width cannot be dragged.

## Measuring

Compare release builds: a debug window shows a yellow `dev build` strip, and
runs several times slower. GPUI times every frame in either app, so the same
line reads both:

```sh
ZED_MEASUREMENTS=1 RUST_LOG=moonframe::perf=debug target/release/sheprd-moonframe
ZED_MEASUREMENTS=1 target/release/sheprd
```

`frame duration` is one frame's draw and present. `moonframe::perf` adds what
each publish and message ran. Machine load moves these a lot, so alternate the
two apps and compare medians.
