/** Sends the whole browser to another address. Kept apart so tests can replace it. */
export function goTo(url: string): void {
	window.location.href = url;
}
