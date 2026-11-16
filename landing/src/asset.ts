/**
 * The address of a file in public/, under the path the page is built for.
 * Vite sets BASE_URL from `base`: "/" normally, "/margincite/" on GitHub Pages.
 */
export function asset(path: string): string {
	return `${import.meta.env.BASE_URL}${path.replace(/^\/+/, "")}`;
}
