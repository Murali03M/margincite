/** The sections of a site, in order. `to` is the path under /sites/:id. */
export const SECTIONS = [
	{ to: "", label: "Overview" },
	{ to: "suggestions", label: "Suggestions" },
	{ to: "checks", label: "Checks" },
	{ to: "keywords", label: "Keywords" },
	{ to: "rankings", label: "Rankings" },
	{ to: "ai", label: "AI Visibility" },
	{ to: "search", label: "Search" },
	{ to: "backlinks", label: "Backlinks" },
	{ to: "pages", label: "Pages" },
	{ to: "connections", label: "Connections" },
	{ to: "settings", label: "Site settings" },
] as const;
