import { useState } from "react";
import type { FormEvent } from "react";
import { Link } from "react-router-dom";
import { errorMessage } from "../api/client";
import { useSaveSiteSettings, useSiteSettings } from "../api/researchHooks";
import type { Device, RankingFrequency, SiteSettings } from "../api/types";
import { Button } from "../ui/Button";
import { Field, SelectField } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { ErrorState, Loading } from "../ui/States";

const PAGE_CAP_MIN = 10;
const PAGE_CAP_MAX = 5000;
const PAGE_CAP_MESSAGE = `Page limit must be a whole number from ${PAGE_CAP_MIN} to ${PAGE_CAP_MAX}.`;

const DEVICES = [
	{ value: "desktop", label: "Desktop" },
	{ value: "mobile", label: "Mobile" },
];
const FREQUENCIES = [
	{ value: "weekly", label: "Every week" },
	{ value: "daily", label: "Every day" },
];

/** True for a whole number inside the range the server accepts. */
export const isValidPageCap = (text: string): boolean => {
	const value = Number(text);
	return text.trim() !== "" && Number.isInteger(value) && value >= PAGE_CAP_MIN && value <= PAGE_CAP_MAX;
};

function Form({ siteId, settings }: { siteId: string; settings: SiteSettings }) {
	const save = useSaveSiteSettings(siteId);
	const [name, setName] = useState(settings.display_name);
	const [cap, setCap] = useState(String(settings.page_cap));
	const [country, setCountry] = useState(settings.country);
	const [language, setLanguage] = useState(settings.language);
	const [device, setDevice] = useState<Device>(settings.device);
	const [track, setTrack] = useState(settings.track_rankings);
	const [frequency, setFrequency] = useState<RankingFrequency>(settings.ranking_frequency);
	const [problem, setProblem] = useState<string | null>(null);
	const [saved, setSaved] = useState(false);

	async function submit(event: FormEvent) {
		event.preventDefault();
		setSaved(false);
		if (!isValidPageCap(cap)) {
			setProblem(PAGE_CAP_MESSAGE);
			return;
		}
		setProblem(null);
		try {
			await save.mutateAsync({
				display_name: name.trim(),
				page_cap: Number(cap),
				country,
				language,
				device,
				track_rankings: track,
				ranking_frequency: frequency,
			});
			setSaved(true);
		} catch (error) {
			setProblem(errorMessage(error));
		}
	}

	return (
		<form aria-label="Site options" onSubmit={(event) => void submit(event)} noValidate className="grid gap-5">
			<div className="grid gap-4 sm:grid-cols-2 sm:items-start">
				<Field label="Name" value={name} maxLength={100} autoComplete="off" onChange={(event) => setName(event.target.value)} />
				<Field
					label="Page limit"
					type="number"
					inputMode="numeric"
					value={cap}
					hint={`How many pages a crawl reads, ${PAGE_CAP_MIN} to ${PAGE_CAP_MAX}. A smaller limit finishes sooner.`}
					onChange={(event) => setCap(event.target.value)}
				/>
			</div>
			<div className="grid gap-3">
				<p className="text-xs leading-relaxed text-ink-muted">
					Where your customers search. Rankings and AI Overviews are checked for this country, language and device, and changing any of them
					starts a new ranking history. Rankings you already have are kept with their old location, never mixed in with the new ones.
				</p>
				<div className="grid gap-4 sm:grid-cols-3">
					<SelectField label="Country" value={country} onChange={setCountry} options={settings.countries.map((c) => ({ value: c.code, label: c.name }))} />
					<SelectField label="Language" value={language} onChange={setLanguage} options={settings.languages.map((l) => ({ value: l.code, label: l.name }))} />
					<SelectField label="Device" value={device} onChange={(value) => setDevice(value as Device)} options={DEVICES} />
				</div>
			</div>
			<div className="grid gap-3 sm:grid-cols-2 sm:items-end">
				<label className="flex items-start gap-2.5 text-sm text-ink">
					<input type="checkbox" aria-label="Track rankings" className="mt-1" checked={track} onChange={(event) => setTrack(event.target.checked)} />
					<span>
						<span className="font-medium">Track rankings</span>
						<span className="block text-xs text-ink-muted">
							Checks where this site ranks for each keyword. It uses paid search data, so add it in{" "}
							<Link to="/settings" className="font-medium text-ink">
								Settings
							</Link>{" "}
							first.
						</span>
					</span>
				</label>
				<SelectField label="How often" value={frequency} onChange={(value) => setFrequency(value as RankingFrequency)} options={FREQUENCIES} disabled={!track} />
			</div>
			{problem && (
				<p role="alert" className="notice">
					{problem}
				</p>
			)}
			<div className="flex flex-wrap items-center gap-3">
				<Button type="submit" busy={save.isPending}>
					Save options
				</Button>
				{saved && (
					<span role="status" className="text-xs font-medium text-ink-soft">
						Saved
					</span>
				)}
			</div>
		</form>
	);
}

/** Name, page limit, search location and rank tracking for one site. */
export function SiteOptionsForm({ siteId }: { siteId: string }) {
	const settings = useSiteSettings(siteId);
	let body;
	if (settings.isPending) body = <Loading label="Loading the site options…" />;
	else if (settings.isError) body = <ErrorState message={errorMessage(settings.error)} onRetry={() => void settings.refetch()} />;
	else body = <Form siteId={siteId} settings={settings.data} />;
	return <Panel title="Site options">{body}</Panel>;
}
