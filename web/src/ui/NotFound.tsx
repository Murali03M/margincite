import { ButtonLink } from "./Button";
import { Empty } from "./States";

/** For an address that matches no screen, and for a resource the server says does not exist. */
export function NotFound({ what = "page" }: { what?: string }) {
	return (
		<Empty title="Not found" action={<ButtonLink to="/">Back to your sites</ButtonLink>}>
			{`That ${what} was not found. It may have been removed, or the address may be wrong.`}
		</Empty>
	);
}
