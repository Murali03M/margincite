import { EXTERNAL_REL, safeHref } from "./safeLink";

/** A crawled address: a link only when it is a safe http or https address, plain text otherwise. */
export function CrawledLink({ url }: { url: string }) {
	const href = safeHref(url);
	if (!href) return <span className="break-all">{url}</span>;
	return (
		<a href={href} rel={EXTERNAL_REL} target="_blank" className="break-all text-course-deep">
			{url}
		</a>
	);
}
