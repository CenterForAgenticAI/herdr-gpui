-- The activity bar: Sheprd's panels, with the counts it reports.
local ICONS = {
	automations = "clock",
	issues = "circle-dot",
	overview = "activity",
	pull_requests = "git-pull-request",
	security = "shield",
	usage = "chart-line",
	workspaces = "square-terminal",
}

-- As Sheprd's own bar: a rail on the edge beside the panel shown, its tile
-- washed in the accent, and the count on the icon's corner.
local function button(id: string, icon: string, label: string?, count: any, selected: boolean, msg: any, theme: any)
	return ui.row {
		class = "activity-button",
		align = "center",
		gap = "none",
		height = 36,
		shrink = false,
		tooltip = label,
		msg = msg,
		to = "provider:sheprd",
		ui.row {
			class = "activity-rail",
			width = 3,
			height = 28,
			shrink = false,
			style = if selected then { background = theme.accent, radius = 2 } else nil,
		},
		ui.column {
			class = "activity-tile",
			align = "center",
			justify = "center",
			width = 32,
			height = 32,
			shrink = false,
			style = { background = if selected and theme.accent then `{theme.accent}30` else nil, radius = 8, margin = { left = 2 } },
			ui.icon { icon, class = "activity-icon", style = { color = if selected then theme.foreground else theme.muted } },
			if count
				then ui.text {
					`{count}`,
					class = "activity-count",
					size = "small",
					float = { at = "top-right", inset = 0 },
					style = { color = theme.accent },
				}
				else false,
		},
	}
end

return mod {
	init = function(previous)
		return previous or {}
	end,

	update = function() end,

	view = function(_, app)
		local sheprd = app.sheprd or { panels = {} }
		local theme = app.sheprd_theme or {}
		local rows = {}
		for _, panel in sheprd.panels do
			table.insert(rows, button(panel.id, ICONS[panel.id] or "circle", panel.label, panel.count, panel.id == sheprd.panel, { panel = panel.id }, theme))
		end
		return ui.column {
			class = "sheprd-activity",
			grow = true,
			gap = "none",
			style = { background = theme.sidebar, border = "right", border_color = theme.active, padding = { y = 6 } },
			ui.column { gap = "none", table.unpack(rows) },
			ui.spacer(),
			button("settings", "settings", "Settings", nil, false, { command = "settings" }, theme),
		}
	end,
}
