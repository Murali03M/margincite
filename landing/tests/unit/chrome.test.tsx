import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { Footer } from "../../src/components/Footer";
import { Hero } from "../../src/components/Hero";
import { Logo } from "../../src/components/Logo";
import { Nav } from "../../src/components/Nav";
import { CourseMap } from "../../src/components/CourseMap";
import { content } from "../../src/content";

it("shows the Margincite name in the logo", () => {
	render(<Logo />);
	expect(screen.getByText("Margincite")).toBeInTheDocument();
});

it("renders the nav links and call to action from content", () => {
	render(<Nav />);
	for (const link of content.nav.links) {
		expect(screen.getAllByRole("link", { name: link.label })[0]).toHaveAttribute("href", link.href);
	}
	expect(screen.getAllByRole("link", { name: "Request early access" })[0]).toHaveAttribute("href", "#early-access");
});

it("opens and closes the mobile menu", async () => {
	render(<Nav />);
	const button = screen.getByRole("button", { name: "Open menu" });
	await userEvent.click(button);
	expect(screen.getByRole("button", { name: "Close menu" })).toHaveAttribute("aria-expanded", "true");
	await userEvent.click(screen.getByRole("button", { name: "Close menu" }));
	expect(screen.getByRole("button", { name: "Open menu" })).toHaveAttribute("aria-expanded", "false");
});

it("renders the hero headline and both buttons", () => {
	render(<Hero />);
	expect(screen.getByRole("heading", { level: 1 }).textContent?.replace(/\s+/g, " ").trim()).toBe("Notes in the margin for your website");
	expect(screen.getByRole("link", { name: /Request early access/ })).toHaveAttribute("href", "#early-access");
	expect(screen.getByRole("link", { name: "See how it works" })).toHaveAttribute("href", "#how");
	expect(screen.getByText("Early access. Works inside Claude.")).toBeInTheDocument();
});

it("names the map and its three fixes for people who cannot see it", () => {
	render(<CourseMap viewBox="0 0 1600 900" />);
	const name = screen.getByRole("img").getAttribute("aria-label") ?? "";
	expect(name).toContain(content.hero.mapLabel);
	content.hero.sketchNotes.forEach((note, i) => expect(name).toContain(`${i + 1}: ${note}`));
});

it("draws control circles as open rings, with nothing painted inside", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" />);
	for (const ring of container.querySelectorAll("[data-control]")) expect(ring).toHaveAttribute("fill", "none");
});

it("never stretches the lettering on a fix tag", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" tags />);
	expect(container.querySelector("[textLength], [textlength]")).toBeNull();
});

it("marks the three example fixes as numbered controls on the map", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" tags />);
	const text = container.textContent ?? "";
	for (const note of content.hero.sketchNotes) expect(text).toContain(note);
	expect(container.querySelectorAll("[data-control]")).toHaveLength(3);
});

it("explains the map with a key in the first screen", () => {
	render(<Hero />);
	for (const entry of content.hero.key) expect(screen.getByText(entry.label)).toBeInTheDocument();
	expect(screen.getByText("Example report")).toBeInTheDocument();
	expect(screen.getByText("An illustration, not a screenshot.")).toBeInTheDocument();
});

it("centres each fix title in its tag, so both sides have the same inset", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" tags />);
	const tags = [...container.querySelectorAll(".course-tag")];
	expect(tags).toHaveLength(3);
	for (const tag of tags) {
		const box = tag.querySelector("rect")!;
		const text = tag.querySelector("text")!;
		const centre = Number(box.getAttribute("x")) + Number(box.getAttribute("width")) / 2;
		expect(text).toHaveAttribute("text-anchor", "middle");
		expect(Number(text.getAttribute("x"))).toBeCloseTo(centre, 3);
	}
});

it("prints the course in plain course purple, with no blending that shifts its colour", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" />);
	const course = container.querySelector("[data-course]")!;
	expect(course).toHaveAttribute("stroke", "var(--color-course)");
	expect(course.getAttribute("style") ?? "").not.toMatch(/mix-blend-mode/);
});

it("knocks the map out behind the start label", () => {
	const { container } = render(<CourseMap viewBox="0 0 1600 900" />);
	expect(container.querySelector("rect[data-start-label]")).toHaveAttribute("fill", "#fff");
});

it("lists the example fixes as text outside the map image", () => {
	render(<Hero />);
	const list = screen.getByRole("list", { name: "example.com: 17 notes" });
	expect(list.querySelectorAll("li")).toHaveLength(3);
});

it("renders the footer without the removed sections", () => {
	const { container } = render(<Footer />);
	expect(screen.getByText("SEO and AI search suggestions for your website.")).toBeInTheDocument();
	expect(container.querySelector('a[href="#pricing"], a[href="#notes"]')).toBeNull();
});

it("leaves no reference text anywhere on the page", () => {
	const { container } = render(<App />);
	expect(container.textContent).not.toMatch(/scribbly|12,400|brainstorm/i);
});
