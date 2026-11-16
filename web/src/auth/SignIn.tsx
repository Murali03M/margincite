import { useState } from "react";
import type { FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { api, ApiError, errorMessage } from "../api/client";
import type { Me } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Logo } from "../ui/Logo";
import { setToken } from "./session";
import { cleanToken } from "./tokenInput";

const NOT_RECOGNISED =
	"That token was not recognised. Use the part before the = sign on the DEV_API_TOKENS line. In the example settings it is dev-token.";

/** The development sign-in: one field, checked against GET /api/v1/me. */
export function SignIn() {
	const navigate = useNavigate();
	const [value, setValue] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [checking, setChecking] = useState(false);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const token = cleanToken(value);
		if (!token) {
			setError("Paste your access token first.");
			return;
		}
		setChecking(true);
		setError(null);
		try {
			await api<Me>("/me", { token });
			setToken(token);
			navigate("/");
		} catch (caught) {
			setError(caught instanceof ApiError && caught.status === 401 ? NOT_RECOGNISED : errorMessage(caught));
		} finally {
			setChecking(false);
		}
	}

	return (
		<div className="min-h-screen flex items-center justify-center bg-canvas p-4">
			<main className="w-full max-w-md rounded-md border border-line bg-raised p-8 grid gap-6">
				<div className="flex justify-center pb-2">
					<Logo />
				</div>
				<div className="text-center">
					<h1 className="text-lg font-medium text-ink">Sign in</h1>
					<p className="mt-2 text-xs text-ink-muted leading-relaxed">
						This is the development sign-in. Open <code className="rounded-md bg-hover px-1 py-0.5 mono text-ink-soft">backend/.env</code>, find the line that starts with{" "}
						<code className="rounded-md bg-hover px-1 py-0.5 mono text-ink-soft">DEV_API_TOKENS=</code>, and paste the part before the next = sign. In the example
						settings that is <code className="rounded-md bg-hover px-1 py-0.5 mono text-ink-soft">dev-token</code>. It is kept in this browser tab only, and it is gone
						when you close the tab.
					</p>
				</div>
				<form onSubmit={submit} className="grid gap-4" noValidate>
					<Field
						label="Access token"
						type="password"
						autoComplete="off"
						value={value}
						onChange={(event) => setValue(event.target.value)}
						error={error}
					/>
					<div className="pt-2">
						<Button type="submit" busy={checking} className="w-full">
							{checking ? "Checking…" : "Sign in"}
						</Button>
					</div>
				</form>
			</main>
		</div>
	);
}
