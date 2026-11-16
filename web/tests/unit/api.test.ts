import { http, HttpResponse } from "msw";
import { api, ApiError } from "../../src/api/client";
import { server } from "../mocks/server";
import { fail, ok } from "../mocks/handlers";

test("api_reads_data_and_meta_from_the_envelope", async () => {
	server.use(http.get("/api/v1/things", () => ok([{ id: 1 }], { total: 9, page: 2, limit: 5 })));
	const result = await api<{ id: number }[]>("/things");
	expect(result.data).toEqual([{ id: 1 }]);
	expect(result.meta).toEqual({ total: 9, page: 2, limit: 5 });
});

test("api sends the token it is given as a bearer header and null meta stays null", async () => {
	let header: string | null = null;
	server.use(
		http.get("/api/v1/things", ({ request }) => {
			header = request.headers.get("authorization");
			return ok({ fine: true });
		}),
	);
	const result = await api("/things", { token: "abc" });
	expect(header).toBe("Bearer abc");
	expect(result.meta).toBeNull();
});

test("api sends a JSON body with its method", async () => {
	let received: unknown = null;
	server.use(
		http.post("/api/v1/things", async ({ request }) => {
			received = await request.json();
			return ok({ made: true }, null, 201);
		}),
	);
	const result = await api<{ made: boolean }>("/things", { method: "POST", body: { url: "example.com" }, token: "t" });
	expect(received).toEqual({ url: "example.com" });
	expect(result.data.made).toBe(true);
});

test("api_throws_the_envelope_error_text", async () => {
	server.use(http.get("/api/v1/things", () => fail(422, "That address is not a public website.")));
	const error = await api("/things").catch((e: unknown) => e);
	expect(error).toBeInstanceOf(ApiError);
	expect((error as ApiError).status).toBe(422);
	expect((error as ApiError).message).toBe("That address is not a public website.");
});

test("api_explains_a_network_failure", async () => {
	server.use(http.get("/api/v1/things", () => HttpResponse.error()));
	const error = (await api("/things").catch((e: unknown) => e)) as ApiError;
	expect(error.status).toBe(0);
	expect(error.message).toBe("Margincite can't be reached. Check that the backend is running.");
});

test("api treats a gateway error with no envelope as the backend being down", async () => {
	server.use(http.get("/api/v1/things", () => new HttpResponse(null, { status: 502 })));
	const error = (await api("/things").catch((e: unknown) => e)) as ApiError;
	expect(error.message).toBe("Margincite can't be reached. Check that the backend is running.");
});

test("api gives a plain message when the body is not an envelope", async () => {
	server.use(http.get("/api/v1/things", () => new HttpResponse("<html>oops</html>", { status: 500 })));
	const error = (await api("/things").catch((e: unknown) => e)) as ApiError;
	expect(error.status).toBe(500);
	expect(error.message).toMatch(/unexpected response/i);
});
