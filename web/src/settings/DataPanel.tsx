import { useState } from "react";
import type { FormEvent } from "react";
import { errorMessage } from "../api/client";
import { useRemoveDataForSeo, useSaveDataForSeo, useTestDataForSeo } from "../api/settingsHooks";
import type { Settings } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";
import { Panel } from "../ui/Panel";
import { formatMoney, shortDate } from "../ui/format";
import { EXTERNAL_REL } from "../ui/safeLink";
import { ConfirmRemove, KeyState, Problem, TestResult } from "./parts";

const UNLOCKS = [
	"Rank tracking: where your site appears in Google for each keyword, and how that changes.",
	"AI Overviews: whether Google's AI answer appears for a keyword and whether it cites you.",
	"Search volume: roughly how many people search each keyword a month.",
	"Keyword ideas: related searches to add to your list.",
];

/** The DataForSEO login and password behind rankings, AI Overviews, volumes and ideas. Write-only. */
export function DataPanel({ data, locked }: { data: Settings["dataforseo"]; locked: boolean }) {
	const save = useSaveDataForSeo();
	const test = useTestDataForSeo();
	const remove = useRemoveDataForSeo();
	const [login, setLogin] = useState("");
	const [password, setPassword] = useState("");
	const [problem, setProblem] = useState<string | null>(null);
	const set = data.login_set && data.password_set;

	async function submit(event: FormEvent) {
		event.preventDefault();
		try {
			await save.mutateAsync({ login: login.trim(), password });
			setProblem(null);
			setLogin("");
			setPassword("");
		} catch (error) {
			setProblem(errorMessage(error));
		} finally {
			save.reset();
		}
	}

	return (
		<Panel title="Search data (DataForSEO)">
			<p className="text-sm leading-relaxed text-ink-soft">
				DataForSEO supplies Google search results. It is paid per use, with a $50 minimum top-up, and Margincite only calls it with
				your own login. What it unlocks:
			</p>
			<ul aria-label="What search data unlocks" className="grid list-disc gap-1 pl-5 text-sm text-ink-soft">
				{UNLOCKS.map((line) => (
					<li key={line}>{line}</li>
				))}
			</ul>
			<p className="text-xs text-ink-muted">
				<a href="https://app.dataforseo.com/api-access" target="_blank" rel={EXTERNAL_REL} className="font-medium text-ink">
					Get your DataForSEO login
				</a>
			</p>
			<TestResult status={data.status} set={set} error={data.error} checkedAt={data.checked_at} />
			{set && data.balance !== null && (
				<p className="text-sm font-medium text-ink">
					{`Balance: ${formatMoney(data.balance)}${data.checked_at ? ` on ${shortDate(data.checked_at)}` : ""}`}
				</p>
			)}
			<p className="text-xs text-ink-soft">
				Login and password: <KeyState set={set} source={data.source} />
			</p>
			<form onSubmit={(event) => void submit(event)} noValidate className="grid gap-3 sm:grid-cols-2 sm:items-start">
				<Field label="DataForSEO login" autoComplete="off" spellCheck={false} value={login} disabled={locked} onChange={(event) => setLogin(event.target.value)} />
				<Field
					label="DataForSEO password"
					type="password"
					autoComplete="off"
					spellCheck={false}
					value={password}
					disabled={locked}
					onChange={(event) => setPassword(event.target.value)}
				/>
				<div className="flex flex-wrap items-start gap-2 sm:col-span-2">
					<Button type="submit" compact busy={save.isPending} disabled={!login.trim() || !password} aria-label="Save DataForSEO">
						Save
					</Button>
					<Button variant="line" compact busy={test.isPending} disabled={!set} aria-label="Test DataForSEO" onClick={() => test.mutate()}>
						Test
					</Button>
				</div>
			</form>
			<Problem error={problem ?? test.error} />
			{data.source === "workspace" && (
				<ConfirmRemove what="DataForSEO login" busy={remove.isPending} error={remove.error} onConfirm={() => remove.mutate()} />
			)}
		</Panel>
	);
}
