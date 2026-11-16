import { render, screen } from "@testing-library/react";
import { App } from "../../src/App";

it("renders only the Margincite sections, in order", () => {
	const { container } = render(<App />);
	const ids = [...container.querySelectorAll("main > section")].map((s) => s.id);
	expect(ids).toEqual(["top", "features", "how", "product", "early-access"]);
});

it("keeps the skip link", () => {
	render(<App />);
	expect(screen.getByRole("link", { name: "Skip to content" })).toHaveAttribute("href", "#top");
});
