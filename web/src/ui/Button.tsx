import type { ButtonHTMLAttributes, ReactNode } from "react";
import { Link } from "react-router-dom";

export type ButtonVariant = "primary" | "line";

type Look = { variant?: ButtonVariant; compact?: boolean; className?: string };

/** The class list for a button look, for anything that has to be a link or another element. */
export function buttonClass({ variant = "primary", compact = false, className = "" }: Look = {}): string {
	return ["btn", variant === "line" ? "btn-line" : "", compact ? "btn-compact" : "", className]
		.filter(Boolean)
		.join(" ");
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & Look & { busy?: boolean };

/** A button. `busy` disables it while a request is running; pass the busy label as the children. */
export function Button({ variant, compact, className, busy, disabled, type = "button", ...rest }: ButtonProps) {
	return (
		<button
			type={type}
			className={buttonClass({ variant, compact, className })}
			disabled={disabled || busy}
			aria-busy={busy || undefined}
			{...rest}
		/>
	);
}

/** A router link that looks like a button. */
export function ButtonLink({ to, children, ...look }: Look & { to: string; children: ReactNode }) {
	return (
		<Link to={to} className={buttonClass(look)}>
			{children}
		</Link>
	);
}
