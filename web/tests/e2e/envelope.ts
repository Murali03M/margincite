export const envelope = (data: unknown, meta: unknown = null, status = 200) => ({
	status,
	contentType: "application/json",
	body: JSON.stringify({ success: true, data, error: null, meta }),
});

export const failure = (status: number, error: string) => ({
	status,
	contentType: "application/json",
	body: JSON.stringify({ success: false, data: null, error, meta: null }),
});
