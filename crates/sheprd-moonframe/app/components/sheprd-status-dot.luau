-- What a worktree's or agent's agent is doing, as Sheprd's sidebar shows it:
-- a fixed slot, so labels line up whatever the status, and a dot that grows
-- while something is happening.

type Props = {
	status: string?,
	theme: any,
}

local function color(status: string?, theme: any)
	if status == "working" then
		return theme.warning
	elseif status == "blocked" then
		return theme.danger
	elseif status == "done" then
		return theme.success
	end
	return theme.muted
end

return component {
	title = "Sheprd Status Dot",
	category = "Sheprd",
	icon = "circle",
	description = "An agent's status as a dot in a fixed slot: small and muted when idle, colored while working, blocked or done.",
	when = "Before a worktree or agent row's label, so every label starts at the same place.",
	props = {
		["status"] = "\"idle\", \"working\", \"blocked\", \"done\" or \"unknown\"",
		["theme"] = "`app.sheprd_theme`, for the colors",
	},
	example = { status = "working", theme = {} },

	build = function(props: Props)
		local quiet = props.status == nil or props.status == "idle" or props.status == "unknown"
		local size = if quiet then 4 else 8
		return ui.row {
			class = "status-slot",
			width = 8,
			height = 16,
			shrink = false,
			align = "center",
			justify = "center",
			ui.row {
				class = "status-dot",
				width = size,
				height = size,
				shrink = false,
				style = { background = color(props.status, props.theme or {}), radius = size / 2 },
			},
		}
	end,
}
