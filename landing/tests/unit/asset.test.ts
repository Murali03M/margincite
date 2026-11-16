import { asset } from "../../src/asset";

afterEach(() => {
	vi.unstubAllEnvs();
});

it("serves files from the site root by default", () => {
	expect(asset("/map/terrain.svg")).toBe("/map/terrain.svg");
	expect(asset("map/terrain.svg")).toBe("/map/terrain.svg");
});

it("serves files from the sub-path the page is built for", () => {
	vi.stubEnv("BASE_URL", "/margincite/");
	expect(asset("/map/terrain.svg")).toBe("/margincite/map/terrain.svg");
});
