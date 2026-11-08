-- The left pane: worktrees, then the agents, as Sheprd's own sidebar draws them.
-- Each row is plain elements, so Inspect reaches its label, branch and pull
-- request. The row's size and colors are here to change.
local icon_button = use "icon-button"

local ROW = 52
local LINK = "provider:sheprd"

local function status_color(status: string, theme: any)
	if status == "working" then
		return theme.warning
	elseif status == "blocked" then
		return theme.danger
	elseif status == "done" then
		return theme.success
	end
	return theme.muted
end

-- A fixed slot, so labels line up whatever the status.
local function dot(status: string, theme: any)
	local quiet = status == "idle" or status == "unknown"
	local size = if quiet then 4 else 8
	return ui.row {
		class = "status-slot",
		width = 8,
		height = 16,
		shrink = false,
		align = "center",
		justify = "center",
		ui.row { class = "status-dot", width = size, height = size, shrink = false, style = { background = status_color(status, theme), radius = size / 2 } },
	}
end

local function lines(workspace: any, theme: any)
	local focused = workspace.focused
	return ui.column {
		grow = true,
		gap = "none",
		ui.text {
			workspace.label,
			class = "worktree-label",
			truncate = true,
			weight = if focused then "bold" else "normal",
			style = { color = if focused then theme.foreground else theme.subtext },
		},
		if workspace.branch and workspace.branch ~= workspace.label
			then ui.text {
				workspace.branch,
				class = "worktree-branch",
				truncate = true,
				style = { color = if focused then theme.accent else theme.muted },
			}
			else false,
	}
end

-- What sits at the end of a row: changes, the dirty mark, the pull request.
local function badges(workspace: any, theme: any)
	return ui.row {
		class = "worktree-badges",
		align = "center",
		shrink = false,
		style = { gap = 6 },
		if workspace.added then ui.text { workspace.added, class = "worktree-added", size = "small", style = { color = theme.success } } else false,
		if workspace.removed then ui.text { workspace.removed, class = "worktree-removed", size = "small", style = { color = theme.danger } } else false,
		if workspace.dirty then ui.icon { "pencil", class = "worktree-dirty", size = "small", style = { color = theme.warning } } else false,
		if workspace.pr then ui.text { workspace.pr, class = "worktree-pr", size = "small", style = { color = theme.accent } } else false,
	}
end

-- A focused row is a card; the border is on it alone, so its padding gives
-- back the pixel and the text stays where it was.
local function card(focused: boolean, theme: any)
	if focused then
		return { background = theme.active, border = true, border_color = theme.muted, radius = 6, padding = { top = 7, x = 11 }, margin = { x = 8 }, gap = 8 }
	end
	return { radius = 6, padding = { top = 8, x = 12 }, margin = { x = 8 }, gap = 8 }
end

local function parent_row(workspace: any, theme: any)
	return ui.row {
		class = "worktree-row",
		state = if workspace.focused then "selected" else nil,
		align = "start",
		height = ROW,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.branch,
		style = card(workspace.focused, theme),
		dot(workspace.agent_status, theme),
		ui.row {
			width = 14,
			height = 16,
			shrink = false,
			align = "center",
			ui.icon { "folder-git-2", class = "worktree-icon", size = "small", style = { color = if workspace.focused then theme.foreground else theme.muted } },
		},
		lines(workspace, theme),
		badges(workspace, theme),
	}
end

-- A linked worktree, on the tree under its checkout: the line runs on past
-- every child but the last.
local function child_row(workspace: any, last: boolean, theme: any)
	return ui.row {
		class = "worktree-row worktree-child",
		state = if workspace.focused then "selected" else nil,
		align = "start",
		gap = "none",
		height = ROW,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.branch,
		style = if workspace.focused
			then { background = theme.active, border = true, border_color = theme.muted, margin = { left = 28, right = 8 }, radius = 6 }
			else { margin = { left = 28, right = 8 }, radius = 6 },
		ui.column { class = "tree-line", width = 1, height = if last then 17 else ROW, shrink = false, style = { background = theme.active } },
		ui.row {
			class = "tree-elbow",
			width = 10,
			height = 1,
			shrink = false,
			style = { background = theme.active, margin = { top = 16 } },
		},
		ui.row { width = 6, shrink = false, style = { margin = { top = 8 } }, dot(workspace.agent_status, theme) },
		ui.row {
			grow = true,
			align = "start",
			style = { gap = 8, padding = { top = 8, left = 8, right = 12 } },
			lines(workspace, theme),
			badges(workspace, theme),
		},
	}
end

-- A row when the sidebar is collapsed: its number on a tile, the label in the
-- tooltip.
local function compact_row(workspace: any, theme: any)
	return ui.column {
		class = "worktree-tile",
		state = if workspace.focused then "selected" else nil,
		align = "center",
		justify = "center",
		height = 36,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.label,
		style = if workspace.focused then { background = theme.active, radius = 6 } else { radius = 6 },
		ui.text {
			`{workspace.number}`,
			class = "worktree-number",
			weight = if workspace.focused then "bold" else "normal",
			style = { color = status_color(workspace.agent_status, theme) },
		},
	}
end

local function agent_row(agent: any, workspace: any, theme: any)
	local focused = workspace ~= nil and workspace.focused
	return ui.row {
		class = "agent-row",
		state = if focused then "selected" else nil,
		align = "start",
		height = ROW,
		shrink = false,
		msg = { focus_pane = agent.pane },
		to = LINK,
		tooltip = agent.title,
		style = card(focused, theme),
		dot(agent.status, theme),
		ui.column {
			grow = true,
			gap = "none",
			ui.text { if workspace then workspace.label else agent.name or "agent", class = "agent-workspace", weight = "bold", truncate = true, style = { color = theme.foreground } },
			ui.row {
				align = "center",
				style = { gap = 4 },
				ui.icon { "asterisk", class = "agent-icon", size = "small", style = { color = theme.muted } },
				ui.text { agent.name or "agent", class = "agent-name", truncate = true, style = { color = theme.muted } },
			},
		},
	}
end

local function link(text: string, command: string, theme: any)
	return ui.row {
		class = "sheprd-link",
		msg = { command = command },
		to = LINK,
		ui.text { text, style = { color = theme.muted } },
	}
end

local function subhead(text: string, theme: any, side: any)
	return ui.row {
		class = "sheprd-subhead",
		align = "center",
		height = 32,
		shrink = false,
		style = { padding = { x = 20 } },
		ui.text { text, style = { color = theme.muted } },
		ui.spacer(),
		side or false,
	}
end

return mod {
	init = function(previous)
		return previous or {}
	end,

	update = function() end,

	view = function(_, app)
		local sheprd = app.sheprd or { workspaces = {}, agents = {} }
		local theme = app.sheprd_theme or {}
		if sheprd.section then
			return ui.column { width = 0 }
		end
		if sheprd.sidebar_visible == false then
			local tiles = {}
			for _, workspace in sheprd.workspaces do
				table.insert(tiles, compact_row(workspace, theme))
			end
			return ui.column {
				class = "sheprd-left",
				grow = true,
				scroll = true,
				padding = "small",
				style = { background = theme.sidebar, gap = 4 },
				table.unpack(tiles),
			}
		end
		local rows, by_id = {}, {}
		for index, workspace in sheprd.workspaces do
			by_id[workspace.id] = workspace
			if workspace.child then
				local following = sheprd.workspaces[index + 1]
				table.insert(rows, child_row(workspace, not (following and following.child), theme))
			else
				table.insert(rows, parent_row(workspace, theme))
			end
		end
		local agents = {}
		for _, agent in sheprd.agents do
			table.insert(agents, agent_row(agent, by_id[agent.workspace], theme))
		end
		return ui.column {
			class = "sheprd-left",
			grow = true,
			gap = "none",
			-- Sheprd's sidebar font: see src/theme.rs.
			style = { background = theme.sidebar, font = "mono" },
			ui.row {
				class = "sheprd-head",
				align = "center",
				height = 32,
				shrink = false,
				style = { padding = { left = 12, right = 8 } },
				ui.text { "WORKSPACES", class = "sheprd-title", weight = "bold", size = "small", style = { color = theme.subtext } },
				ui.spacer(),
				icon_button { icon = "panel-left", size = "small", tooltip = "Toggle the sidebar", msg = { command = "toggle_sidebar" }, to = LINK },
			},
			subhead("Spaces", theme, icon_button { icon = "search", size = "small", tooltip = "Search", msg = { command = "command_palette" }, to = LINK }),
			ui.column {
				grow = true,
				scroll = true,
				gap = "none",
				ui.column { shrink = false, gap = "none", table.unpack(rows) },
			},
			ui.row {
				class = "sheprd-left-actions",
				align = "center",
				height = 28,
				shrink = false,
				style = { padding = { x = 20 } },
				link("new", "new_workspace", theme),
				ui.spacer(),
				link("menu", "command_palette", theme),
			},
			ui.row { class = "sheprd-rule", height = 1, shrink = false, style = { background = theme.active } },
			subhead("Agents", theme),
			ui.column {
				height = math.min(#agents * ROW + 4, 240),
				shrink = false,
				scroll = true,
				gap = "none",
				ui.column { shrink = false, gap = "none", table.unpack(agents) },
			},
		}
	end,
}
