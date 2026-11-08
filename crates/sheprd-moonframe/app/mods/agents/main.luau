-- The left pane's agents, under the worktrees (`mods/worktrees`): one row
-- per agent, with the worktree it works in. A click shows its pane. The card
-- the selected row shows is in `style.luau`.
local status_dot = use "sheprd-status-dot"
local subhead = use "sheprd-subhead"

local ROW = 52
-- The tallest the list grows before it scrolls, leaving the worktrees room.
local MOST = 240
local LINK = "provider:sheprd"

local function agent_row(agent: any, workspace: any, theme: any)
	return ui.row {
		class = "agent-row sheprd-card",
		selected = workspace ~= nil and workspace.focused,
		align = "start",
		height = ROW,
		shrink = false,
		msg = { focus_pane = agent.pane },
		to = LINK,
		tooltip = agent.title,
		status_dot { status = agent.status, theme = theme },
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

return mod {
	init = function(previous)
		return previous or {}
	end,

	update = function() end,

	view = function(_, app)
		local sheprd = app.sheprd or { workspaces = {}, agents = {} }
		local theme = app.sheprd_theme or {}
		-- A page in the sidebar's place, or the sidebar collapsed to tiles.
		if sheprd.section or sheprd.sidebar_visible == false then
			return ui.column { height = 0 }
		end
		local by_id = {}
		for _, workspace in sheprd.workspaces do
			by_id[workspace.id] = workspace
		end
		local rows = {}
		for _, agent in sheprd.agents do
			table.insert(rows, agent_row(agent, by_id[agent.workspace], theme))
		end
		return ui.column {
			class = "sheprd-agents",
			shrink = false,
			gap = "none",
			-- Sheprd's sidebar font: see src/theme.rs.
			style = { background = theme.sidebar, font = "mono" },
			ui.row { class = "sheprd-rule", height = 1, shrink = false, style = { background = theme.active } },
			subhead { text = "Agents", theme = theme },
			ui.column {
				height = math.min(#rows * ROW + 4, MOST),
				shrink = false,
				scroll = true,
				gap = "none",
				ui.column { shrink = false, gap = "none", table.unpack(rows) },
			},
		}
	end,
}
