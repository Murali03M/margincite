import { render, screen } from "@testing-library/react";
import { App } from "../../src/App";

it("keeps the same sections, in order", () => {
	const { container } = render(<App />);
	const ids = [...container.querySelectorAll("main > section")].map((s) => s.id);
	expect(ids).toEqual(["top", "features", "layers", "how", "product", "faq", "get-it"]);
});

it("keeps the skip link", () => {
	render(<App />);
	expect(screen.getByRole("link", { name: "Skip to content" })).toHaveAttribute("href", "#top");
});

it("no longer asks for an email address", () => {
	const { container } = render(<App />);
	expect(container.querySelector("form, input")).toBeNull();
	expect(container.textContent).not.toMatch(/early access|works inside claude/i);
});
