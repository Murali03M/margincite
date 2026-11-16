import { useEffect, useState } from "react";
import { applyTheme, browserStorage, type Theme } from "../lib/theme";

function SunIcon() {
	return (
		<svg viewBox="0 0 24 24" className="h-5 w-5" aria-hidden>
			<circle cx="12" cy="12" r="4.2" fill="none" stroke="currentColor" strokeWidth="2" />
			<path
				d="M12 2.5v2.2M12 19.3v2.2M2.5 12h2.2M19.3 12h2.2M5.3 5.3l1.6 1.6M17.1 17.1l1.6 1.6M5.3 18.7l1.6-1.6M17.1 6.9l1.6-1.6"
				fill="none"
				stroke="currentColor"
				strokeWidth="2"
				strokeLinecap="round"
			/>
		</svg>
	);
}

function MoonIcon() {
	return (
		<svg viewBox="0 0 24 24" className="h-5 w-5" aria-hidden>
			<path d="M20 14.6A8.2 8.2 0 0 1 9.4 4a8.2 8.2 0 1 0 10.6 10.6Z" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round" />
		</svg>
	);
}

/** Switches between the dark room (default) and the light room. Shows the room it will switch to. */
export function ThemeToggle({ className = "", withLabel = false }: { className?: string; withLabel?: boolean }) {
	const [theme, setTheme] = useState<Theme>("dark");

	useEffect(() => {
		setTheme(document.documentElement.dataset.theme === "light" ? "light" : "dark");
	}, []);

	const next: Theme = theme === "dark" ? "light" : "dark";

	return (
		<button
			type="button"
			aria-label={`Switch to ${next} theme`}
			title={`Switch to ${next} theme`}
			onClick={() => {
				applyTheme(document.documentElement, next, browserStorage());
				setTheme(next);
			}}
			className={"btn btn-line btn-compact " + (withLabel ? "" : "px-2 ") + className}
		>
			{next === "light" ? <SunIcon /> : <MoonIcon />}
			{withLabel && <span>{`${next} theme`}</span>}
		</button>
	);
}
