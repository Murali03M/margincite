import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Faq } from "../../src/components/Faq";
import { Features } from "../../src/components/Features";
import { GetIt } from "../../src/components/GetIt";
import { Hero } from "../../src/components/Hero";
import { HowItWorks } from "../../src/components/HowItWorks";
import { Layers } from "../../src/components/Layers";
import { ProductDetail } from "../../src/components/ProductDetail";
import { REPO_URL, content } from "../../src/content";

it("renders the hero headline, both buttons and the status line", () => {
	render(<Hero />);
	expect(screen.getByRole("heading", { level: 1 }).textContent?.replace(/\s+/g, " ").trim()).toBe("Every fix comes with its evidence");
	expect(screen.getByRole("link", { name: /Get it on GitHub/ })).toHaveAttribute("href", REPO_URL);
	expect(screen.getByRole("link", { name: "See how it works" })).toHaveAttribute("href", "#how");
	expect(screen.getByText(content.hero.status)).toBeInTheDocument();
});

it("names the illustrated finding for people who cannot see it, and labels it an example", () => {
	render(<Hero />);
	expect(screen.getByRole("figure", { name: content.hero.exhibit.label })).toBeInTheDocument();
	expect(screen.getByText("Example")).toBeInTheDocument();
	expect(screen.getByText(content.hero.exhibit.note)).toBeInTheDocument();
});

it("places one numbered marker on each blocked line", () => {
	const { container } = render(<Hero />);
	const markers = [...container.querySelectorAll("[data-marker]")].map((m) => m.getAttribute("data-marker"));
	expect(markers).toEqual(["1", "2", "3"]);
});

it("lists every feature with what it serves and what it needs", () => {
	render(<Features />);
	expect(screen.getByRole("heading", { level: 2, name: "What it does today" })).toBeInTheDocument();
	const rows = within(screen.getByRole("table")).getAllByRole("row").slice(1);
	expect(rows).toHaveLength(content.features.items.length);
	content.features.items.forEach((item, i) => {
		expect(within(rows[i]).getByRole("heading", { level: 3, name: item.title })).toBeInTheDocument();
		expect(within(rows[i]).getByText(item.needs)).toBeInTheDocument();
		for (const area of item.serves) expect(within(rows[i]).getByText(area)).toBeInTheDocument();
	});
});

it("shows the three steps as an ordered list, in order", () => {
	render(<HowItWorks />);
	const items = within(screen.getByRole("list")).getAllByRole("listitem");
	expect(items.map((li) => within(li).getByRole("heading").textContent)).toEqual(["Run it", "Connect your providers", "Add a site, read the fixes"]);
});

it("shows one example suggestion with all four parts", () => {
	render(<ProductDetail />);
	expect(screen.getByRole("heading", { level: 3, name: content.product.note.title })).toBeInTheDocument();
	for (const part of content.product.note.parts) {
		expect(screen.getByText(part.label)).toBeInTheDocument();
		expect(screen.getByText(part.body)).toBeInTheDocument();
	}
});

it("states the honesty line and what is coming later", () => {
	render(<ProductDetail />);
	expect(screen.getByText(content.product.honesty)).toBeInTheDocument();
	const coming = screen.getByRole("list", { name: "Coming later" });
	expect(within(coming).getAllByRole("listitem")).toHaveLength(content.product.coming.items.length);
});

it("prints the install commands as text and links to GitHub", () => {
	render(<GetIt />);
	const code = screen.getByRole("region", { name: content.getIt.commandsLabel });
	for (const line of content.getIt.commands) expect(code).toHaveTextContent(line);
	expect(screen.getByRole("link", { name: /Get it on GitHub/ })).toHaveAttribute("href", REPO_URL);
});

it("copies the commands and says so", async () => {
	const writeText = vi.fn(async () => {});
	const user = userEvent.setup();
	Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
	render(<GetIt />);
	await user.click(screen.getByRole("button", { name: "Copy commands" }));
	expect(writeText).toHaveBeenCalledWith(content.getIt.commands.join("\n"));
	expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument();
});

it("says how to recover when copying fails", async () => {
	const user = userEvent.setup();
	Object.defineProperty(navigator, "clipboard", {
		value: { writeText: vi.fn(async () => Promise.reject(new Error("denied"))) },
		configurable: true,
	});
	render(<GetIt />);
	await user.click(screen.getByRole("button", { name: "Copy commands" }));
	await act(async () => {});
	expect(screen.getByRole("status")).toHaveTextContent(content.getIt.copy.failed);
});

it("lists the key features in a strip under the hero", () => {
	render(<Hero />);
	const strip = screen.getByRole("list", { name: content.hero.strip.label });
	expect(within(strip).getAllByRole("listitem").map((li) => li.textContent)).toEqual([...content.hero.strip.items]);
});

it("compares SEO, AIO and GEO side by side", () => {
	render(<Layers />);
	expect(screen.getByRole("heading", { level: 2, name: content.layers.heading })).toBeInTheDocument();
	const table = screen.getByRole("table");
	for (const column of content.layers.columns) expect(within(table).getByRole("columnheader", { name: new RegExp(column.name) })).toBeInTheDocument();
	expect(within(table).getAllByRole("row")).toHaveLength(content.layers.rows.length + 1);
});

it("shows a sample ranked list, labelled as an example", () => {
	render(<ProductDetail />);
	const list = screen.getByRole("list", { name: content.product.sample.caption });
	expect(within(list).getAllByRole("listitem")).toHaveLength(content.product.sample.rows.length);
	expect(screen.getAllByText("Example").length).toBeGreaterThanOrEqual(2);
});

it("answers the FAQ in the HTML, readable without JavaScript", () => {
	const { container } = render(<Faq />);
	expect(screen.getByRole("heading", { level: 2, name: content.faq.heading })).toBeInTheDocument();
	const items = container.querySelectorAll("details");
	expect(items).toHaveLength(content.faq.items.length);
	content.faq.items.forEach((item, i) => {
		expect(items[i].querySelector("summary")).toHaveTextContent(item.question);
		expect(items[i]).toHaveTextContent(item.answer);
	});
});
