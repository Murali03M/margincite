import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { vi } from "vitest";
import { Cta } from "../../src/components/Cta";

const endpoint = "https://forms.example.test/f/abc";
const email = () => screen.getByLabelText("Email address");
const submit = () => screen.getByRole("button", { name: "Request early access" });

beforeEach(() => {
	vi.spyOn(console, "error").mockImplementation(() => {});
});

it("is a real form that works without JavaScript", () => {
	const { container } = render(<Cta endpoint={endpoint} />);
	const form = container.querySelector("form")!;
	expect(form).toHaveAttribute("action", endpoint);
	expect(form).toHaveAttribute("method", "post");
	expect(email()).toHaveAttribute("name", "email");
});

it("confirms after a successful request", async () => {
	const fetchFn = vi.fn().mockResolvedValue(new Response("{}", { status: 200 }));
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(await screen.findByRole("status")).toHaveTextContent("Thanks. You are on the list.");
});

it("shows an error and keeps the typed email when the service is unreachable", async () => {
	const fetchFn = vi.fn().mockRejectedValue(new TypeError("failed to fetch"));
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(await screen.findByRole("alert")).toHaveTextContent("We could not reach the sign-up service. Please try again.");
	expect(email()).toHaveValue("a@b.co");
});

it("shows the invalid-email message and sends nothing", async () => {
	const fetchFn = vi.fn();
	render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "nope");
	await userEvent.click(submit());
	expect(await screen.findByRole("alert")).toHaveTextContent("That email address does not look right.");
	expect(fetchFn).not.toHaveBeenCalled();
});

it("says sign-up is not connected when there is no endpoint", async () => {
	render(<Cta endpoint={undefined} fetchFn={vi.fn()} />);
	await userEvent.type(email(), "a@b.co");
	await userEvent.click(submit());
	expect(await screen.findByRole("alert")).toHaveTextContent("Sign-up is not connected yet. Please try again later.");
});

it("sends one request when the form is submitted twice before the first answer", async () => {
	let release!: (r: Response) => void;
	const fetchFn = vi.fn().mockReturnValue(new Promise<Response>((r) => { release = r; }));
	const { container } = render(<Cta endpoint={endpoint} fetchFn={fetchFn} />);
	await userEvent.type(email(), "a@b.co");
	const form = container.querySelector("form")!;
	fireEvent.submit(form);
	fireEvent.submit(form);
	expect(await screen.findByRole("button", { name: "Sending…" })).toBeDisabled();
	release(new Response("{}", { status: 200 }));
	await screen.findByRole("status");
	expect(fetchFn).toHaveBeenCalledTimes(1);
});

it("hides the honeypot field from people", () => {
	const { container } = render(<Cta endpoint={endpoint} />);
	const trap = container.querySelector('input[name="_gotcha"]')!;
	expect(container.querySelector('input[name="company"]')).toBeNull();
	expect(trap).toHaveAttribute("data-1p-ignore");
	expect(trap).toHaveAttribute("data-lpignore", "true");
	expect(trap).toHaveAttribute("tabindex", "-1");
	expect(trap).toHaveAttribute("autocomplete", "off");
	expect(trap.closest("[aria-hidden='true']")).not.toBeNull();
});
