import { cleanToken } from "../../src/auth/tokenInput";

test.each([
	["dev-token", "dev-token"],
	["  dev-token \n", "dev-token"],
	["dev-token=you@example.com", "dev-token"],
	["DEV_API_TOKENS=dev-token=you@example.com", "dev-token"],
	["DEV_API_TOKENS=dev-token=you@example.com,other=someone@example.com", "dev-token"],
	['"dev-token"', "dev-token"],
	["export DEV_API_TOKENS=dev-token=you@example.com", "dev-token"],
	["", ""],
])("cleanToken(%j) is %j", (typed, expected) => {
	expect(cleanToken(typed)).toBe(expected);
});

test("a token that itself contains no equals sign is left alone", () => {
	expect(cleanToken("abc.DEF_123-xyz")).toBe("abc.DEF_123-xyz");
});
