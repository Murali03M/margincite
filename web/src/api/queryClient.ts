import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { clearToken } from "../auth/session";
import { ApiError } from "./client";

/** Any 401 means the token is gone or wrong: drop it, and the app shows sign-in. */
function dropTokenOn401(error: unknown) {
	if (error instanceof ApiError && error.status === 401) clearToken();
}

/** Errors have a Retry button on screen, so requests are not retried behind the person's back. */
export function makeQueryClient(): QueryClient {
	return new QueryClient({
		queryCache: new QueryCache({ onError: dropTokenOn401 }),
		mutationCache: new MutationCache({ onError: dropTokenOn401 }),
		defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
	});
}
