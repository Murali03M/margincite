import { safeHref } from "../../src/ui/safeLink";

test("safeHref_allows_http_and_https_only", () => {
	expect(safeHref("https://example.com/a?b=1")).toBe("https://example.com/a?b=1");
	expect(safeHref("http://example.com/")).toBe("http://example.com/");
	expect(safeHref("javascript:alert(1)")).toBeNull();
	expect(safeHref("data:text/html,x")).toBeNull();
	expect(safeHref("//evil.test")).toBeNull();
	expect(safeHref("/relative")).toBeNull();
	expect(safeHref("")).toBeNull();
});
