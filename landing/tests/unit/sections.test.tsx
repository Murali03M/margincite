import { render, screen, within } from "@testing-library/react";
import { Features } from "../../src/components/Features";
import { HowItWorks } from "../../src/components/HowItWorks";
import { ProductDetail } from "../../src/components/ProductDetail";
import { content } from "../../src/content";

it("lists the four features", () => {
	render(<Features />);
	expect(screen.getByRole("heading", { level: 2, name: "What it does today" })).toBeInTheDocument();
	for (const item of content.features.items) {
		expect(screen.getByRole("heading", { level: 3, name: item.title })).toBeInTheDocument();
		expect(screen.getByText(item.body)).toBeInTheDocument();
	}
});

it("shows the three steps as an ordered list, in order", () => {
	render(<HowItWorks />);
	const items = within(screen.getByRole("list")).getAllByRole("listitem");
	expect(items.map((li) => within(li).getByRole("heading").textContent)).toEqual(["Add your site", "Read the notes", "Fix and re-check"]);
});

it("shows one example suggestion with its three parts", () => {
	render(<ProductDetail />);
	expect(screen.getByText("Example")).toBeInTheDocument();
	expect(screen.getByRole("heading", { level: 3, name: "AI crawlers are blocked in robots.txt" })).toBeInTheDocument();
	for (const part of content.product.note.parts) {
		expect(screen.getByText(part.label)).toBeInTheDocument();
		expect(screen.getByText(part.body)).toBeInTheDocument();
	}
});

it("states the honesty line and what is coming later", () => {
	render(<ProductDetail />);
	expect(screen.getByText("Margincite measures and reports. It does not promise rankings or citations.")).toBeInTheDocument();
	const coming = screen.getByRole("list", { name: "Coming later" });
	expect(within(coming).getAllByRole("listitem")).toHaveLength(2);
});
