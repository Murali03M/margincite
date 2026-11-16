import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { http } from "msw";
import { getToken } from "../../src/auth/session";
import { server } from "../mocks/server";
import { fail, GOOD_TOKEN, ok } from "../mocks/handlers";
import { renderApp } from "../render";

const emptySites = http.get("/api/v1/sites", () => ok([]));

test("signing_in_with_a_good_token_enters_the_app", async () => {
	server.use(emptySites);
	renderApp("/", { token: null });
	await userEvent.type(screen.getByLabelText(/access token/i), GOOD_TOKEN);
	await userEvent.click(screen.getByRole("button", { name: /sign in/i }));
	expect(await screen.findByText("owner@example.com")).toBeInTheDocument();
	expect(getToken()).toBe(GOOD_TOKEN);
});

test("pasting the whole settings line still signs in with just the token", async () => {
	server.use(emptySites);
	renderApp("/", { token: null });
	await userEvent.type(screen.getByLabelText(/access token/i), `DEV_API_TOKENS=${GOOD_TOKEN}=owner@example.com`);
	await userEvent.click(screen.getByRole("button", { name: /sign in/i }));
	expect(await screen.findByText("owner@example.com")).toBeInTheDocument();
	expect(getToken()).toBe(GOOD_TOKEN);
});

test("the sign-in page says which part of the settings line to paste", () => {
	renderApp("/", { token: null });
	expect(screen.getByText(/DEV_API_TOKENS=/)).toBeInTheDocument();
	expect(screen.getByText("dev-token")).toBeInTheDocument();
});

test("the sign-in page says it is the development sign-in and where the token lives", () => {
	renderApp("/", { token: null });
	expect(screen.getByText(/development sign-in/i)).toBeInTheDocument();
	expect(screen.getByText(/backend\/\.env/)).toBeInTheDocument();
});

test("a_bad_token_shows_the_not_recognised_message", async () => {
	renderApp("/", { token: null });
	await userEvent.type(screen.getByLabelText(/access token/i), "wrong");
	await userEvent.click(screen.getByRole("button", { name: /sign in/i }));
	expect(await screen.findByText(/That token was not recognised\./)).toBeInTheDocument();
	expect(screen.getByText(/part before the = sign/)).toBeInTheDocument();
	expect(getToken()).toBeNull();
});

test("sign-in explains a backend that cannot be reached", async () => {
	server.use(http.get("/api/v1/me", () => fail(503, "Down for maintenance.")));
	renderApp("/", { token: null });
	await userEvent.type(screen.getByLabelText(/access token/i), GOOD_TOKEN);
	await userEvent.click(screen.getByRole("button", { name: /sign in/i }));
	expect(await screen.findByText("Down for maintenance.")).toBeInTheDocument();
});

test("a_401_on_any_request_returns_to_sign_in", async () => {
	// The shell asks who is signed in; the server says the token has expired.
	server.use(emptySites, http.get("/api/v1/me", () => fail(401, "Token expired.")));
	renderApp("/");
	expect(await screen.findByLabelText(/access token/i)).toBeInTheDocument();
	expect(getToken()).toBeNull();
});

test("signing_out_clears_the_token", async () => {
	server.use(emptySites);
	renderApp("/");
	await userEvent.click(await screen.findByRole("button", { name: /sign out/i }));
	expect(await screen.findByLabelText(/access token/i)).toBeInTheDocument();
	expect(getToken()).toBeNull();
});

test("blocked_session_storage_does_not_crash_the_app", async () => {
	server.use(emptySites);
	const blocked = {
		getItem() {
			throw new Error("blocked");
		},
		setItem() {
			throw new Error("blocked");
		},
		removeItem() {
			throw new Error("blocked");
		},
	};
	vi.stubGlobal("sessionStorage", blocked);
	renderApp("/", { token: null });
	await userEvent.type(screen.getByLabelText(/access token/i), GOOD_TOKEN);
	await userEvent.click(screen.getByRole("button", { name: /sign in/i }));
	expect(await screen.findByText("owner@example.com")).toBeInTheDocument();
	await userEvent.click(screen.getByRole("button", { name: /sign out/i }));
	await waitFor(() => expect(screen.getByLabelText(/access token/i)).toBeInTheDocument());
});
