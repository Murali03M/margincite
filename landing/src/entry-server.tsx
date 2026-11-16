import { StrictMode } from "react";
import { renderToString } from "react-dom/server";
import { App } from "./App";

export { buildHead, buildRobots, buildSitemap, normalizeSiteUrl } from "./seo";

/** Renders the whole page to HTML, so every word is present before JavaScript runs. */
export function render(): string {
	return renderToString(
		<StrictMode>
			<App />
		</StrictMode>,
	);
}
