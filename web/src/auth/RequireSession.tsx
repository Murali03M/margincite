import { useSyncExternalStore } from "react";
import type { ReactNode } from "react";
import { getToken, subscribeToken } from "./session";
import { SignIn } from "./SignIn";

/** Shows its children only while a token is held. With none, or after any 401, it shows sign-in. */
export function RequireSession({ children }: { children: ReactNode }) {
	const token = useSyncExternalStore(subscribeToken, getToken, () => null);
	return token ? <>{children}</> : <SignIn />;
}
