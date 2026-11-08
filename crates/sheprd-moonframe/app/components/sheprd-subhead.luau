-- A section's name over its list in the left pane, with an optional control
-- at its end.

type Props = {
	text: string,
	side: any,
	theme: any,
}

return component {
	title = "Sheprd Subhead",
	category = "Sheprd",
	icon = "heading",
	description = "A quiet section name over a list, with room for one control at its end.",
	when = "Over each list of the left pane: Spaces, Agents.",
	props = {
		["text"] = "the section's name",
		["side"] = "an element at its end, such as an icon button",
		["theme"] = "`app.sheprd_theme`, for the colors",
	},
	example = { text = "Agents", theme = {} },

	build = function(props: Props)
		local theme = props.theme or {}
		return ui.row {
			class = "sheprd-subhead",
			align = "center",
			height = 32,
			shrink = false,
			style = { padding = { x = 20 } },
			ui.text { props.text, style = { color = theme.muted } },
			ui.spacer(),
			props.side or false,
		}
	end,
}
