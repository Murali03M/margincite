/** A date for people, in UTC so it reads the same everywhere: "1 Nov 2026". */
export function shortDate(iso: string): string {
	const date = new Date(iso);
	if (Number.isNaN(date.getTime())) return "Unknown";
	return date.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
}

/** A whole number with thousands separators: 1200 becomes "1,200". */
export function formatNumber(value: number): string {
	return value.toLocaleString("en-US", { maximumFractionDigits: 1 });
}

/** A share such as 40 or 33.3, shown as "40%" or "33.3%". */
export function formatPercent(value: number): string {
	return `${formatNumber(value)}%`;
}

/** A cost in US dollars with cents: 0.8 becomes "$0.80". */
export function formatMoney(value: number): string {
	return `$${value.toFixed(2)}`;
}

/** "LOW" becomes "Low". */
export function titleCase(value: string): string {
	return value.charAt(0).toUpperCase() + value.slice(1).toLowerCase();
}
