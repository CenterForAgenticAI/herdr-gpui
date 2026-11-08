# Terminals that herdr-core (Herdr 2) runs, drawn by the app in tabs.

core-terminals-open-workspace = Open a workspace first
# A new tab on an SSH host, cloud machine, or WSL distribution without
# herdr-core runs in the host's Herdr instead.
core-terminals-host-without-core = herdr-core is not installed on this host, so the tab runs in its Herdr. Choose Install Herdr 2 in the host's menu to add it
core-terminals-too-many-tabs = Too many tabs are open
core-terminals-connecting = Connecting to herdr-core...
# $reason is the error, in English or from herdr-core.
core-terminals-unavailable = herdr-core is unavailable: { $reason }
core-terminals-start-failed = Could not start a terminal: { $reason }
core-terminals-exited = Exited with code { $code }
core-terminals-ended = Ended
# What an agent in a split pane reports, beside its title.
core-terminals-state-blocked = blocked
core-terminals-state-done = done
core-terminals-state-idle = idle
core-terminals-state-working = working
core-terminals-mirror-needs-pane = Click a Herdr 2 pane to mirror first
# Shown on a watch-only view of a terminal another view types into.
core-terminals-watching = Watching
# The sidebar's list of herdr-core panes, under the agents.
core-terminals-sidebar = Herdr 2
# Installing herdr-core (Herdr 2) on a saved device, from its menu.
# $device is the device's name; $version is herdr-core's version.
core-terminals-install-started = Installing Herdr 2 on { $device }...
core-terminals-install-installed = Herdr 2 (herdr-core { $version }) is installed on { $device }
core-terminals-install-present = { $device } already has Herdr 2
# $reason is why, in English or from the device.
core-terminals-install-failed = Could not install Herdr 2 on { $device }: { $reason }
core-terminals-install-local = this computer runs the herdr-core bundled with Sheprd
