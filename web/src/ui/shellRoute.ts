import { SECTIONS } from "../sites/sections";

export type PageName = { siteId: string | null; name: string };

const SECTION_NAME = new Map<string, string>(SECTIONS.map((section) => [section.to, section.label]));

/** Which site (if any) and which page an address belongs to, for the title bar and the sidebar. */
export function pageName(pathname: string, search: string): PageName {
	const parts = pathname.split("/").filter(Boolean);
	const [first, second, third] = parts;
	if (parts.length === 0) return { siteId: null, name: "Sites" };
	if (first === "sites" && second === "new") return { siteId: null, name: "Add site" };
	if (first === "sites" && second) {
		return { siteId: second, name: SECTION_NAME.get(third ?? "") ?? "Not found" };
	}
	if (first === "settings") return { siteId: null, name: "Settings" };
	if (first === "suggestions") return { siteId: new URLSearchParams(search).get("site"), name: "Suggestions" };
	if (first === "pages") return { siteId: null, name: "Page" };
	return { siteId: null, name: "Not found" };
}
