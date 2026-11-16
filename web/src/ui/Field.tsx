import { useId } from "react";
import type { InputHTMLAttributes, ReactNode } from "react";

type FieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "id"> & {
	label: string;
	/** Plain help shown under the label. */
	hint?: string;
	/** The reason the value was refused. Shown under the field; the typed value stays. */
	error?: ReactNode;
};

/** A labelled text field. The label is always visible and tied to the input. */
export function Field({ label, hint, error, ...input }: FieldProps) {
	const id = useId();
	const hintId = `${id}-hint`;
	const errorId = `${id}-error`;
	const described = [hint ? hintId : "", error ? errorId : ""].filter(Boolean).join(" ") || undefined;
	return (
		<div className="grid gap-1.5">
			<label htmlFor={id} className="text-xs font-medium text-ink-soft">
				{label}
			</label>
			{hint && (
				<p id={hintId} className="text-xs leading-relaxed text-ink-muted">
					{hint}
				</p>
			)}
			<input
				{...input}
				id={id}
				className="field"
				aria-invalid={error ? true : undefined}
				aria-describedby={described}
			/>
			{error && (
				<p id={errorId} role="alert" className="notice mt-1">
					{error}
				</p>
			)}
		</div>
	);
}

type SelectFieldProps = {
	label: string;
	value: string;
	onChange: (value: string) => void;
	options: readonly { value: string; label: string }[];
	disabled?: boolean;
};

/** A labelled choice from a short list. The label is always visible. */
export function SelectField({ label, value, onChange, options, disabled }: SelectFieldProps) {
	const id = useId();
	return (
		<div className="flex flex-col gap-1.5">
			<label htmlFor={id} className="text-xs font-medium text-ink-soft">
				{label}
			</label>
			<div className="relative">
				<select
					id={id}
					className="field appearance-none pr-8"
					value={value}
					disabled={disabled}
					onChange={(event) => onChange(event.target.value)}
				>
					{options.map((option) => (
						<option key={option.value} value={option.value}>
							{option.label}
						</option>
					))}
				</select>
				<div className="pointer-events-none absolute inset-y-0 right-0 flex items-center pr-2.5 text-ink-muted">
					<svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4" aria-hidden>
						<path fillRule="evenodd" d="M5.22 8.22a.75.75 0 0 1 1.06 0L10 11.94l3.72-3.72a.75.75 0 1 1 1.06 1.06l-4.25 4.25a.75.75 0 0 1-1.06 0L5.22 9.28a.75.75 0 0 1 0-1.06Z" clipRule="evenodd" />
					</svg>
				</div>
			</div>
		</div>
	);
}
