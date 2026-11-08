-- The left pane's worktrees, as Sheprd's own sidebar draws them: the head,
-- then each checkout with its linked worktrees under it. Each row is plain
-- elements, so Inspect reaches its label, branch and pull request. The agents
-- under them are their own mod (`mods/agents`); the card a selected row
-- shows is in `style.luau`.
local icon_button = use "icon-button"
local status_dot = use "sheprd-status-dot"
local subhead = use "sheprd-subhead"

local ROW = 52
local LINK = "provider:sheprd"
-- The theme color a status shows in, as `sheprd-status-dot` colors it.
local TONES = { working = "warning", blocked = "danger", done = "success" }

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

local function parent_row(workspace: any, theme: any)
	return ui.row {
		class = "worktree-row sheprd-card",
		selected = workspace.focused,
		align = "start",
		height = ROW,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.branch,
		status_dot { status = workspace.agent_status, theme = theme },
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
		selected = workspace.focused,
		align = "start",
		gap = "none",
		height = ROW,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.branch,
		ui.column { class = "tree-line", width = 1, height = if last then 17 else ROW, shrink = false, style = { background = theme.active } },
		ui.row {
			class = "tree-elbow",
			width = 10,
			height = 1,
			shrink = false,
			style = { background = theme.active, margin = { top = 16 } },
		},
		ui.row { width = 6, shrink = false, style = { margin = { top = 8 } }, status_dot { status = workspace.agent_status, theme = theme } },
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
		selected = workspace.focused,
		align = "center",
		justify = "center",
		height = 36,
		shrink = false,
		msg = { focus_workspace = workspace.id },
		to = LINK,
		tooltip = workspace.label,
		ui.text {
			`{workspace.number}`,
			class = "worktree-number",
			weight = if workspace.focused then "bold" else "normal",
			style = { color = TONES[workspace.agent_status] and theme[TONES[workspace.agent_status]] or theme.muted },
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

return mod {
	init = function(previous)
		return previous or {}
	end,

	update = function() end,

	view = function(_, app)
		local sheprd = app.sheprd or { workspaces = {} }
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
				class = "sheprd-worktrees",
				grow = true,
				scroll = true,
				padding = "small",
				style = { background = theme.sidebar, gap = 4 },
				table.unpack(tiles),
			}
		end
		local rows = {}
		for index, workspace in sheprd.workspaces do
			if workspace.child then
				local following = sheprd.workspaces[index + 1]
				table.insert(rows, child_row(workspace, not (following and following.child), theme))
			else
				table.insert(rows, parent_row(workspace, theme))
			end
		end
		return ui.column {
			class = "sheprd-worktrees",
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
			subhead {
				text = "Spaces",
				theme = theme,
				side = icon_button { icon = "search", size = "small", tooltip = "Search", msg = { command = "command_palette" }, to = LINK },
			},
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
		}
	end,
}
