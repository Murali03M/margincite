import { useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { ApiError, errorMessage } from "../api/client";
import { fetchSites, keys, useCreateSite } from "../api/hooks";
import type { Site } from "../api/types";
import { Button } from "../ui/Button";
import { Field } from "../ui/Field";

/** The host a typed address points at, for matching it against sites already added. */
function hostOf(typed: string): string | null {
	try {
		const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(typed) ? typed : `https://${typed}`;
		return new URL(withScheme).hostname.toLowerCase().replace(/^www\./, "");
	} catch {
		return null;
	}
}

/** The site the person already added, if the sites list has it. */
async function findExisting(
	client: ReturnType<typeof useQueryClient>,
	typed: string,
): Promise<Site | null> {
	const host = hostOf(typed);
	if (!host) return null;
	try {
		const sites = await client.fetchQuery({ queryKey: keys.sites, queryFn: fetchSites, staleTime: 0 });
		return sites.find((site) => site.host.toLowerCase().replace(/^www\./, "") === host) ?? null;
	} catch {
		return null;
	}
}

/** One field: the address of the site. */
export function AddSitePage() {
	const navigate = useNavigate();
	const client = useQueryClient();
	const create = useCreateSite();
	const [address, setAddress] = useState("");
	const [error, setError] = useState<ReactNode>(null);

	async function submit(event: FormEvent) {
		event.preventDefault();
		const url = address.trim();
		if (!url) {
			setError("Enter the address of your website.");
			return;
		}
		setError(null);
		try {
			const created = await create.mutateAsync({ url });
			navigate(`/sites/${created.id}`);
		} catch (caught) {
			if (caught instanceof ApiError && caught.status === 409) {
				const existing = await findExisting(client, url);
				setError(
					<>
						That site is already added.{" "}
						{existing ? (
							<Link to={`/sites/${existing.id}`} className="font-medium text-ink underline">
								Open {existing.host}
							</Link>
						) : (
							<Link to="/" className="font-medium text-ink underline">
								See your sites
							</Link>
						)}
					</>,
				);
			} else {
				setError(errorMessage(caught));
			}
		}
	}

	return (
		<div className="mx-auto grid max-w-xl gap-6">
			<div className="rounded-md border border-line bg-raised p-6 sm:p-8 grid gap-5">
				<div>
					<h1 className="text-lg font-medium text-ink">Add a site</h1>
					<p className="text-xs text-ink-muted mt-1">Margincite will index the site and generate actionable SEO & GEO recommendations.</p>
				</div>
				<form onSubmit={submit} className="grid gap-5" noValidate>
					<Field
						label="Website address"
						hint="For example example.com. Margincite crawls from this address and stays on this site."
						type="text"
						inputMode="url"
						autoComplete="url"
						placeholder="example.com"
						value={address}
						onChange={(event) => setAddress(event.target.value)}
						error={error}
					/>
					<div className="flex flex-wrap items-center gap-3 pt-2">
						<Button type="submit" busy={create.isPending}>
							{create.isPending ? "Adding…" : "Add site"}
						</Button>
						<Link to="/" className="btn btn-line">
							Cancel
						</Link>
					</div>
				</form>
			</div>
		</div>
	);
}
