import { useRef, useState, type FormEvent } from "react";
import { FinishSymbol } from "./symbols";
import { content, type ErrorReason } from "../content";
import { submitEarlyAccess } from "../lib/earlyAccess";
import { asset } from "../asset";

type Status =
	| { kind: "idle" }
	| { kind: "sending" }
	| { kind: "sent" }
	| { kind: "error"; reason: ErrorReason };

type CtaProps = { endpoint?: string; fetchFn?: typeof fetch };

type FormProps = {
	endpoint: string | undefined;
	email: string;
	honeypot: string;
	status: Status;
	onEmail: (value: string) => void;
	onHoneypot: (value: string) => void;
	onSubmit: (event: FormEvent) => void;
};

const cta = content.cta;

function EarlyAccessForm({ endpoint, email, honeypot, status, onEmail, onHoneypot, onSubmit }: FormProps) {
	const failed = status.kind === "error";
	const sending = status.kind === "sending";
	return (
		<form action={endpoint} method="post" onSubmit={onSubmit} className="mt-7" noValidate>
			<div className="flex flex-col gap-3 sm:flex-row">
				<label htmlFor="cta-email" className="sr-only">
					{cta.emailLabel}
				</label>
				<input
					id="cta-email"
					name="email"
					type="email"
					required
					autoComplete="email"
					placeholder={cta.placeholder}
					value={email}
					onChange={(e) => onEmail(e.target.value)}
					aria-invalid={failed}
					aria-describedby={failed ? "cta-error" : undefined}
					className="field flex-1"
				/>
				<button type="submit" disabled={sending} className="btn btn-on-course shrink-0">
					{sending ? cta.sending : cta.button}
				</button>
			</div>

			{/* Honeypot: people never see or reach this field, bots fill it in. Its
			    name is one no autofill or password manager recognises, so a real
			    visitor's browser never fills it for them. */}
			<div aria-hidden="true" className="sr-only">
				<label htmlFor="cta-trap">Leave this empty</label>
				<input
					id="cta-trap"
					name="_gotcha"
					type="text"
					tabIndex={-1}
					autoComplete="off"
					data-1p-ignore
					data-lpignore="true"
					value={honeypot}
					onChange={(e) => onHoneypot(e.target.value)}
				/>
			</div>

			{status.kind === "error" && (
				<p id="cta-error" role="alert" className="mt-4 rounded-[4px] bg-open px-4 py-2.5 font-semibold text-ink">
					{cta.errors[status.reason]}
				</p>
			)}
		</form>
	);
}

export function Cta({
	endpoint = import.meta.env.VITE_EARLY_ACCESS_ENDPOINT,
	fetchFn = fetch,
}: CtaProps) {
	const [email, setEmail] = useState("");
	const [honeypot, setHoneypot] = useState("");
	const [status, setStatus] = useState<Status>({ kind: "idle" });
	const inFlight = useRef(false);

	const onSubmit = async (event: FormEvent) => {
		event.preventDefault();
		if (inFlight.current) return;
		inFlight.current = true;
		setStatus({ kind: "sending" });
		const result = await submitEarlyAccess({ email, honeypot }, { endpoint, fetchFn });
		inFlight.current = false;
		setStatus(result.ok ? { kind: "sent" } : { kind: "error", reason: result.reason });
	};

	return (
		<section
			id="early-access"
			className="on-course contours bg-course py-24 text-white md:py-32"
			style={{
				["--contour-image" as string]: `url(${asset("map/contours-white.svg")})`,
				["--contour-strength" as string]: 0.2,
				["--contour-shift" as string]: "62% 78%",
			}}
		>
			<div className="wrap grid items-center gap-x-16 gap-y-10 lg:grid-cols-[minmax(0,6fr)_minmax(0,5fr)]">
				<div className="flex items-start gap-6">
					<FinishSymbol className="hidden h-28 w-28 shrink-0 sm:block" />
					<h2>{cta.heading}</h2>
				</div>

				<div>
					<p className="lead">{cta.body}</p>
					{status.kind === "sent" ? (
						<p role="status" className="mt-7 flex items-center gap-3 rounded-[4px] bg-sheet px-5 py-4 font-semibold text-course-deep">
							<FinishSymbol className="h-8 w-8 shrink-0" />
							{cta.success}
						</p>
					) : (
						<EarlyAccessForm
							endpoint={endpoint}
							email={email}
							honeypot={honeypot}
							status={status}
							onEmail={setEmail}
							onHoneypot={setHoneypot}
							onSubmit={onSubmit}
						/>
					)}
					<p className="mt-5 text-[1rem] text-course-pale">{cta.footnote}</p>
				</div>
			</div>
		</section>
	);
}
