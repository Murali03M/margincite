import type { ReactElement } from "react";
import { render } from "@testing-library/react";
import { QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import { App } from "../src/App";
import { makeQueryClient } from "../src/api/queryClient";
import { routerFuture } from "../src/routerFuture";
import { setToken } from "../src/auth/session";
import { GOOD_TOKEN } from "./mocks/handlers";

/** Renders the whole app at a route. Signed in by default; pass `token: null` to start signed out. */
export function renderApp(path = "/", { token = GOOD_TOKEN }: { token?: string | null } = {}) {
	if (token) setToken(token);
	return renderAt(<App />, path);
}

/** Renders one component inside the same providers the app uses, at a route. */
export function renderAt(ui: ReactElement, path = "/") {
	const client = makeQueryClient();
	return {
		client,
		...render(
			<QueryClientProvider client={client}>
				<MemoryRouter initialEntries={[path]} future={routerFuture}>{ui}</MemoryRouter>
			</QueryClientProvider>,
		),
	};
}
