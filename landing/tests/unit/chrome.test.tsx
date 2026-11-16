import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToString } from "react-dom/server";
import { Footer } from "../../src/components/Footer";
import { Logo } from "../../src/components/Logo";
import { Nav } from "../../src/components/Nav";
import { VisitCounter } from "../../src/components/VisitCounter";
import { REPO_URL, content } from "../../src/content";

it("shows the Margincite name in the logo", () => {
	render(<Logo />);
	expect(screen.getByText("Margincite")).toBeInTheDocument();
});

it("renders the nav links and the GitHub button", () => {
	render(<Nav load={() => new Promise(() => {})} />);
	for (const link of content.nav.links) {
		expect(screen.getAllByRole("link", { name: link.label })[0]).toHaveAttribute("href", link.href);
	}
	expect(screen.getAllByRole("link", { name: /Get it on GitHub/ })[0]).toHaveAttribute("href", REPO_URL);
});

it("opens and closes the mobile menu", async () => {
	render(<Nav load={() => new Promise(() => {})} />);
	await userEvent.click(screen.getByRole("button", { name: "Open menu" }));
	expect(screen.getByRole("button", { name: "Close menu" })).toHaveAttribute("aria-expanded", "true");
	await userEvent.click(screen.getByRole("button", { name: "Close menu" }));
	expect(screen.getByRole("button", { name: "Open menu" })).toHaveAttribute("aria-expanded", "false");
});

it("prints no visit count in the HTML, so no number is ever made up", () => {
	expect(renderToString(<VisitCounter load={async () => 99} />)).toBe("");
});

it("shows the count in fixed digit places once it arrives", async () => {
	let finish: (value: number | null) => void = () => {};
	render(<VisitCounter load={() => new Promise((resolve) => (finish = resolve))} />);
	expect(screen.getByText("Counting visits")).toBeInTheDocument();
	await act(async () => finish(1234));
	expect(screen.getByText("1,234 visits to this page")).toBeInTheDocument();
	expect(document.querySelectorAll("[data-digit]")).toHaveLength(6);
	expect(screen.getByText("Visits")).toBeInTheDocument();
});

it("removes the counter when the count cannot be loaded", async () => {
	const { container } = render(<VisitCounter load={async () => null} />);
	await act(async () => {});
	expect(container).toBeEmptyDOMElement();
});

it("credits the licence, the source and the counting service in the footer", () => {
	render(<Footer />);
	expect(screen.getByText(content.footer.blurb)).toBeInTheDocument();
	expect(screen.getByText(content.footer.licence)).toBeInTheDocument();
	expect(screen.getByRole("link", { name: "Source on GitHub" })).toHaveAttribute("href", REPO_URL);
	expect(screen.getByRole("link", { name: "Visit count by Abacus" })).toHaveAttribute("href", content.footer.counter.href);
});
