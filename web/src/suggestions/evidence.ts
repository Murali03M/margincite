const isPrimitive = (value: unknown): value is string | number | boolean | null | undefined =>
	value === null || value === undefined || typeof value !== "object";

function sentence(key: string): string {
	const text = key.replace(/_/g, " ").trim();
	return text.charAt(0).toUpperCase() + text.slice(1);
}

function show(value: string | number | boolean | null | undefined): string {
	if (value === null || value === undefined || value === "") return "none";
	if (typeof value === "boolean") return value ? "yes" : "no";
	return String(value);
}

function lines(label: string, value: unknown): string[] {
	if (isPrimitive(value)) return [`${label}: ${show(value)}`];
	if (Array.isArray(value)) {
		if (value.length === 0) return [`${label}: none`];
		if (value.every(isPrimitive)) return [`${label}: ${value.map(show).join(", ")}`];
		return value.flatMap((entry) => lines(label, entry));
	}
	const entries = Object.entries(value as Record<string, unknown>);
	if (entries.length === 0) return [`${label}: none`];
	return entries.flatMap(([key, inner]) => lines(`${label} ${key.replace(/_/g, " ")}`, inner));
}

/**
 * Turns an item's evidence into readable lines such as "Title length: 0".
 * Everything comes out as plain text; nothing is ever shown as raw JSON.
 */
export function evidenceLines(evidence: Record<string, unknown>): string[] {
	return Object.entries(evidence).flatMap(([key, value]) => lines(sentence(key), value));
}
