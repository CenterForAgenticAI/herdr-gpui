-- The window's layout: Sheprd's activity bar, sidebar, main area and status
-- bar, each a region placed here. Move them, or lay out your own around them,
-- from this file or ~/.config/sheprd-moonframe/main.luau. Sheprd reports the
-- sizes it wants, so toggling its sidebar or status bar resizes these.

local function region(name: string)
	return ui.view { name = "sheprd", id = name, props = { region = name } }
end

return mod {
	window = { titlebar = "hidden", traffic_lights = { x = 9, y = 9 } },

	init = function()
		-- Never restored: Sheprd sends them as soon as it starts.
		return { activity_bar = 44, header = 34, left = 300, status_bar = 24 }
	end,

	update = function(state, msg)
		if type(msg) == "table" and msg.left then
			state.activity_bar = msg.activity_bar
			state.header = msg.header
			state.left = msg.left
			state.status_bar = msg.status_bar
		end
	end,

	view = function(state)
		return ui.column {
			grow = true,
			gap = "none",
			ui.row {
				grow = true,
				gap = "none",
				ui.column {
					width = state.left,
					shrink = false,
					gap = "none",
					ui.row { height = state.header, shrink = false, gap = "none", region "header" },
					ui.row {
						grow = true,
						gap = "none",
						ui.column { width = state.activity_bar, shrink = false, gap = "none", region "activity" },
						ui.column { grow = true, gap = "none", region "sidebar" },
					},
				},
				ui.column {
					grow = true,
					gap = "none",
					ui.column { grow = true, gap = "none", region "main" },
					ui.row { height = state.status_bar, shrink = false, gap = "none", region "status" },
				},
			},
			-- Menus and toasts, in the window's coordinates: over everything.
			ui.row { float = "fill", gap = "none", region "overlay" },
			-- The sparkles (the built-in feedback mod): say what to change, and
			-- an agent changes this layout, asking before each edit.
			ui.row {
				class = "feedback-corner",
				float = { at = "bottom-right", inset = 40 },
				ui.mount "feedback",
			},
		}
	end,
}
