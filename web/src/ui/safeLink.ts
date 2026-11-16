/** A crawled address may become a link only when it is an absolute http or https address. */
export function safeHref(url: string): string | null {
	try {
		const parsed = new URL(url);
		return parsed.protocol === "http:" || parsed.protocol === "https:" ? url : null;
	} catch {
		return null;
	}
}

/** The rel value every link to a crawled address carries. */
export const EXTERNAL_REL = "noopener noreferrer nofollow";
