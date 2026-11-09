# Terminals that herdr-core (Herdr 2) runs, drawn by the app in tabs.

core-terminals-open-workspace = Open a workspace first
# The app keeps Herdr 2 workspaces for this computer only.
core-terminals-workspaces-local-only = Herdr 2 workspaces are only on this computer
# A new tab on an SSH host, cloud machine, or WSL distribution without
# herdr-core runs in the host's Herdr instead.
core-terminals-host-without-core = herdr-core is not installed on this host, so the tab runs in its Herdr. Choose Install Herdr 2 in the host's menu to add it
core-terminals-too-many-tabs = Too many tabs are open
# An automation's agent runs in this computer's Herdr 2, not on the host shown.
core-terminals-this-computer = This agent runs on this computer; show it to open the agent
core-terminals-connecting = Connecting to herdr-core...
# $reason is the error, in English or from herdr-core.
core-terminals-unavailable = herdr-core is unavailable: { $reason }
core-terminals-start-failed = Could not start a terminal: { $reason }
core-terminals-exited = Exited with code { $code }
core-terminals-ended = Ended
# Shown over a terminal's last frame while the app reconnects to herdr-core.
core-terminals-reconnecting = Reconnecting...
core-terminals-input-dropped = Not connected to herdr-core: input not sent
core-terminals-pane-own-tab = Another window split this tab while it was full, so its new pane opened in a tab of its own
# What an agent in a split pane reports, beside its title.
core-terminals-state-blocked = blocked
core-terminals-state-done = done
core-terminals-state-idle = idle
core-terminals-state-working = working
core-terminals-mirror-needs-pane = Click a Herdr 2 pane to mirror first
# A pane or tab command a Herdr 2 pane cannot run.
core-terminals-unsupported = Herdr 2 panes cannot do this yet
# Shown on a watch-only view of a terminal another view types into.
core-terminals-watching = Watching
# The sidebar's list of herdr-core panes, under the agents.
core-terminals-sidebar = Herdr 2
# Installing herdr-core (Herdr 2) on a saved device, from its menu.
# $device is the device's name; $version is herdr-core's version.
core-terminals-install-started = Installing Herdr 2 on { $device }...
core-terminals-install-installed = Herdr 2 (herdr-core { $version }) is installed on { $device }
core-terminals-install-present = { $device } already has Herdr 2
core-terminals-install-running = Herdr 2 is already being installed on { $device }
# $reason is why, in English or from the device.
core-terminals-install-failed = Could not install Herdr 2 on { $device }: { $reason }
core-terminals-install-local = this computer runs the herdr-core bundled with Sheprd
