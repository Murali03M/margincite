import { useState } from "react";
import { errorMessage } from "../api/client";
import { useChooseProperty, useGoogleProperties } from "../api/hooks";
import { Button } from "../ui/Button";
import { ErrorState, Loading } from "../ui/States";

/** Lists the Search Console properties the person can read and saves the chosen one. */
export function PropertyChooser({ siteId, current, onDone }: { siteId: string; current: string | null; onDone: () => void }) {
	const properties = useGoogleProperties(siteId, true);
	const choose = useChooseProperty(siteId);
	const [picked, setPicked] = useState<string | null>(current);

	if (properties.isPending) return <Loading label="Loading your Search Console properties…" />;
	if (properties.isError) return <ErrorState message={errorMessage(properties.error)} onRetry={() => void properties.refetch()} />;
	if (properties.data.length === 0) {
		return (
			<p className="notice">
				Google lists no properties you can read. Add and verify this site in Google Search Console first, then come back.
			</p>
		);
	}

	async function save() {
		if (!picked) return;
		try {
			await choose.mutateAsync(picked);
			onDone();
		} catch {
			// The refusal is shown from the mutation's state below.
		}
	}

	return (
		<fieldset className="grid gap-3">
			<legend className="mb-1 text-sm font-medium text-ink">Choose the Search Console property for this site</legend>
			{properties.data.map((row) => (
				<label key={row.property} className="flex cursor-pointer items-start gap-2.5 rounded-md border border-line p-3 text-sm has-[:checked]:border-ink has-[:checked]:bg-hover">
					<input type="radio" name="property" className="mt-1" checked={picked === row.property} onChange={() => setPicked(row.property)} />
					<span className="min-w-0 break-all">
						{row.property} <span className="text-xs text-ink-muted">({row.permission})</span>
					</span>
				</label>
			))}
			<div>
				<Button disabled={!picked} busy={choose.isPending} onClick={() => void save()}>
					Use this property
				</Button>
			</div>
			{choose.isError && (
				<p role="alert" className="notice">
					{errorMessage(choose.error)}
				</p>
			)}
		</fieldset>
	);
}
