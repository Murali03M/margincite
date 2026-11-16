/**
 * The page's two rooms. Dark is the default, also without JavaScript; a visitor
 * can switch to light and the choice is kept in this browser. index.html runs a
 * copy of readTheme before the page paints, so there is no flash of the wrong room.
 */

export type Theme = "dark" | "light";

export const THEME_KEY = "margincite:theme";

type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

export function readTheme(storage: Storage | undefined): Theme {
	try {
		return storage?.getItem(THEME_KEY) === "light" ? "light" : "dark";
	} catch {
		return "dark";
	}
}

export function applyTheme(root: HTMLElement, theme: Theme, storage: Storage | undefined): void {
	root.dataset.theme = theme;
	try {
		storage?.setItem(THEME_KEY, theme);
	} catch {
		// Storage is blocked. The choice holds until the page is closed.
	}
}

export function browserStorage(): Storage | undefined {
	try {
		return window.localStorage;
	} catch {
		return undefined;
	}
}
