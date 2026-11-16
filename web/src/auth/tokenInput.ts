/**
 * People paste the token in several forms: on its own, as the `token=email`
 * pair, or as the whole `DEV_API_TOKENS=...` line from the settings file.
 * This returns just the token from any of them.
 */
export function cleanToken(typed: string): string {
	const withoutQuotes = typed.trim().replace(/^["']|["']$/g, "");
	const withoutName = withoutQuotes.replace(/^(export\s+)?DEV_API_TOKENS\s*=\s*/i, "");
	const firstPair = withoutName.split(",")[0] ?? "";
	return (firstPair.split("=")[0] ?? "").trim();
}
