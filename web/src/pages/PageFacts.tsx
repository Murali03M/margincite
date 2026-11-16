import type { ReactNode } from "react";
import type { PageDetail } from "../api/types";
import { CrawledLink } from "../ui/CrawledLink";

function Fact({ label, children }: { label: string; children: ReactNode }) {
	return (
		<li className="border-b border-line py-3 last:border-b-0">
			<div className="grid gap-x-4 gap-y-1 sm:grid-cols-[12rem_1fr] items-baseline">
				<span className="text-xs font-medium text-ink-muted">{label}</span>
				<div className="break-words text-sm text-ink">{children}</div>
			</div>
		</li>
	);
}

const yesNo = (value: boolean) => (value ? "Yes" : "No");

/** The @type names of the structured data blocks, when they have one. */
function structuredTypes(blocks: unknown[]): string[] {
	return blocks.flatMap((block) => {
		const type = (block as { "@type"?: unknown } | null)?.["@type"];
		return typeof type === "string" ? [type] : [];
	});
}

function StructuredData({ blocks }: { blocks: unknown[] }) {
	if (blocks.length === 0) return <span className="text-ink-muted">None found</span>;
	const types = structuredTypes(blocks);
	const count = `${blocks.length} ${blocks.length === 1 ? "block" : "blocks"}`;
	return (
		<span className="inline-flex items-center gap-2">
			<span className="tag">
				{count}
			</span>
			{types.length > 0 && <span className="text-xs text-ink-soft mono">({types.join(", ")})</span>}
		</span>
	);
}

/** Everything the crawl recorded about one page, as ruled rows of plain text. */
export function PageFacts({ page }: { page: PageDetail }) {
	const facts = page.facts;
	return (
		<ul aria-label="Page facts" className="divide-y divide-line">
			<Fact label="Address">
				<CrawledLink url={page.url} />
			</Fact>
			<Fact label="Status">
				<span className="font-medium text-ink">{page.status_code ?? "Not read"}</span>
				{page.fetch_error ? <span className="text-ink-soft ml-1">({page.fetch_error})</span> : ""}
			</Fact>
			<Fact label="Response time">
				<span className="mono text-ink-soft">{page.response_ms === null ? "Unknown" : `${page.response_ms} ms`}</span>
			</Fact>
			<Fact label="Needed JavaScript">
				<span className={page.needed_javascript ? "text-ink-soft font-medium" : "text-ink-soft"}>{yesNo(page.needed_javascript)}</span>
			</Fact>
			{facts && (
				<>
					<Fact label="Title">{facts.title ?? <span className="text-ink-muted">None</span>}</Fact>
					<Fact label="Description">{facts.meta_description ?? <span className="text-ink-muted">None</span>}</Fact>
					<Fact label="Canonical">{facts.canonical ? <CrawledLink url={facts.canonical} /> : <span className="text-ink-muted">None</span>}</Fact>
					<Fact label="Word count"><span className="mono">{facts.word_count}</span></Fact>
					<Fact label="Structured data">
						<StructuredData blocks={facts.structured_data} />
					</Fact>
					<Fact label="Headings">
						{facts.headings.length === 0 ? (
							<span className="text-ink-muted">None</span>
						) : (
							<ul className="grid gap-1 mt-1">
								{facts.headings.map((heading, index) => (
									<li key={`${index}-${heading.text}`} className="mono text-ink-soft bg-canvas px-2 py-1 rounded border border-line">
										{`H${heading.level} ${heading.text}`}
									</li>
								))}
							</ul>
						)}
					</Fact>
				</>
			)}
		</ul>
	);
}
