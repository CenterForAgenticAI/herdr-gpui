-- Rows answer the pointer as Sheprd's do, and the selected one is a card.
-- Colors are the toolkit's, which follow Sheprd's theme: `muted` is its
-- selection, `muted-foreground` its quiet text.
return style {
	rules = {
		-- A worktree or agent row. The selected card's border is on it alone,
		-- so its padding gives back the pixel and the text stays put.
		[".sheprd-card"] = { radius = 6, padding = { top = 8, x = 12 }, margin = { x = 8 }, gap = 8 },
		[".sheprd-card:selected"] = {
			background = "muted",
			border = "all",
			border_color = "muted-foreground",
			padding = { top = 7, x = 11 },
		},
		-- A linked worktree under its checkout, its tree line inside the card.
		[".worktree-child"] = { radius = 6, margin = { left = 28, right = 8 } },
		[".worktree-child:selected"] = { background = "muted", border = "all", border_color = "muted-foreground" },
		[".worktree-tile"] = { radius = 6 },
		[".worktree-tile:selected"] = { background = "muted" },
		[".worktree-row:hover"] = { background = "secondary" },
		[".agent-row:hover"] = { background = "secondary" },
		[".activity-tile:hover"] = { background = "secondary" },
		[".status-item:hover"] = { opacity = 0.75 },
		[".sheprd-link:hover"] = { opacity = 0.75 },
	},
}
