# Herdr 2 link-handler adapters

Sheprd can run explicitly configured commands for **link-modifier clicks in
Herdr 2 desktop panes**. Nothing runs on hover, plain clicks, selection, or
terminal output alone. Herdr 1 continues to use its daemon's plugin handlers.

Add adapters to `config-gpui.local.toml`. This is trusted executable
configuration: select the command yourself, rather than copying commands from
terminal output. No Herdr plugin registry, manifest, startup hook, or build hook
is automatically imported.

```toml
[[link_handlers]]
plugin = "example.notes"
id = "markdown"
host = "local"
pattern = '^file://.*\.md$'
directory = "/absolute/path/to/my-plugin"
command = ["/absolute/path/to/my-plugin/open-link"]
enabled = true
```

`command` is an argv array, not shell text. A shell is used only if explicitly
named in that array. The URL is never substituted into argv or shell code.
`directory` is the trusted command directory on the target machine, not a cwd
provided by terminal output. Use absolute paths for both it and the executable.

Handlers match the original OSC 8 hyperlink target, including `file://` links
on remote hosts, or a printed web URL. Printed file paths do not invoke handlers.
Enabled handlers are sorted by `plugin` lexically, then retain their declaration
order within that plugin, as Herdr 1 does. The first matching regex claims the
click. Exit failure, launch failure, cancellation, or connection failure never
tries another handler or opens the fallback as well. With no match, normal
browser, file, external-scheme confirmation, and Alt target selection apply.
An unclaimed remote file is never opened from the local disk.

`host` is required: `local` for this computer, otherwise the exact saved device
scope. There is no wildcard or remote-to-local fallback. Each remote adapter's
command and directory must exist on that host. SSH, cloud and WSL use the
existing host transport; native local commands use argv on Windows too. A
changed or removed device refuses activation from its old view. Host selection
cannot redirect an already accepted click. One handler runs per window at a
time; repeated clicks while it runs are consumed. Output is bounded to 64 KiB
and runtime to 30 seconds; closing the owning window cancels its worker.

## Command contract and compatibility limits

Commands receive `HERDR_PLUGIN_ID`, `HERDR_PLUGIN_LINK_HANDLER_ID`,
`HERDR_PLUGIN_CLICKED_URL`, and `HERDR_PLUGIN_CONTEXT_JSON` containing
`invocation_source = "link_click"`, `clicked_url`, and `link_handler_id`.
`SHEPRD_LINK_TERMINAL_ID` and `SHEPRD_LINK_HOST` identify the clicked core
terminal and its host. They are data, not Herdr 1 pane IDs. Inherited pane and
API socket routing is cleared.

This supports standalone commands or purpose-built adapters. It is **not full
Herdr plugin runtime compatibility**: plugins requiring `HERDR_SOCKET_PATH`,
Herdr 1 workspace/tab/pane IDs, plugin popups, lifecycle hooks, installation
leases, or daemon APIs need adaptation. No fake Herdr 1 identity is supplied.
Plugin/platform selection is explicit through per-host configuration; manifests
and their platform inheritance are not loaded. Phone/web clicks, including with
the desktop closed, do not invoke these desktop adapters.

For existing plugins, inspect their action command and dependencies first. Copy
only a compatible link action into an adapter, preserving plugin IDs and handler
order; install its dependencies on the named host. Do not copy startup/build
commands. Transparent compatibility needs a separately specified host-side
plugin runtime/API contract; it is not achieved by sending a core terminal ID
to a Herdr 1 daemon.

## CLI

The existing configuration CLI reads, validates, replaces, and removes the array:

```sh
sheprd config get link_handlers --json
sheprd config check --json
sheprd config set link_handlers '[]' --json
sheprd config unset link_handlers --json
```

There is deliberately no handler-execution CLI: activation requires a terminal
link click. Configuration reload applies to subsequent clicks.
