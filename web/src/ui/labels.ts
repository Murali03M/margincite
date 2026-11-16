import type { Category } from "../api/types";

const CATEGORIES: Record<Category, string> = {
	technical: "Technical",
	ai_readiness: "AI readiness",
	content: "Content",
};

export const categoryLabel = (category: Category): string => CATEGORIES[category] ?? category;

/** How serious a check is, 1 to 3. */
export const severityLabel = (severity: number): string =>
	severity >= 3 ? "High impact" : severity === 2 ? "Medium impact" : "Low impact";

/** How much work the fix is, 1 to 3. */
export const effortLabel = (effort: number): string =>
	effort >= 3 ? "Big job" : effort === 2 ? "Some work" : "Quick fix";
