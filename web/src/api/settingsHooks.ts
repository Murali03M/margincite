import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, query } from "./client";
import { keys } from "./hooks";
import type { ProviderId, ProviderOut, Settings, UsageRow } from "./types";

export const settingsKey = ["settings"] as const;
export const usageKey = (days: number) => ["usage", days] as const;

/** Every credential and option, as "is it set, and where does it come from". Never a secret value. */
export function useSettings() {
	return useQuery({ queryKey: settingsKey, queryFn: async () => (await api<Settings>("/settings")).data });
}

/** Saved settings change what the rest of the app can do, so everything that depends on them is refreshed. */
function useRefreshSettings() {
	const client = useQueryClient();
	return () => {
		void client.invalidateQueries({ queryKey: settingsKey });
		void client.invalidateQueries({ queryKey: keys.integrations });
	};
}

/** Puts one provider's fresh state into the settings the screen already has. */
function useReplaceProvider() {
	const client = useQueryClient();
	const refresh = useRefreshSettings();
	return (provider: ProviderOut) => {
		client.setQueryData<Settings>(settingsKey, (old) =>
			old ? { ...old, providers: old.providers.map((p) => (p.provider === provider.provider ? provider : p)) } : old,
		);
		refresh();
	};
}

export type ProviderInput = { api_key?: string; model?: string };

/**
 * Saves a key and/or model, then the server tests the key. The key only travels in this request body.
 * The caller should call `reset()` once it has the result so the key is not kept in memory by the mutation.
 */
export function useSaveProvider(provider: ProviderId) {
	const replace = useReplaceProvider();
	return useMutation({
		mutationFn: async (input: ProviderInput) =>
			(await api<ProviderOut>(`/settings/providers/${provider}`, { method: "PUT", body: input })).data,
		onSuccess: replace,
	});
}

export function useTestProvider(provider: ProviderId) {
	const replace = useReplaceProvider();
	return useMutation({
		mutationFn: async () => (await api<ProviderOut>(`/settings/providers/${provider}/test`, { method: "POST" })).data,
		onSuccess: replace,
	});
}

export function useRemoveProvider(provider: ProviderId) {
	const refresh = useRefreshSettings();
	return useMutation({
		mutationFn: async () => (await api<unknown>(`/settings/providers/${provider}`, { method: "DELETE" })).data,
		onSuccess: refresh,
	});
}

/** A save or removal of one group of settings that answers with nothing the screen needs: it just reloads them. */
function useGroupMutation<I>(path: string, method: "PUT" | "POST" | "DELETE") {
	const refresh = useRefreshSettings();
	return useMutation({
		mutationFn: async (input: I) => (await api<unknown>(path, { method, body: method === "DELETE" ? undefined : input })).data,
		onSuccess: refresh,
	});
}

export const useSaveGoogle = () => useGroupMutation<{ client_id: string; client_secret: string }>("/settings/google", "PUT");
export const useRemoveGoogle = () => useGroupMutation<void>("/settings/google", "DELETE");
export const useSaveDataForSeo = () => useGroupMutation<{ login: string; password: string }>("/settings/dataforseo", "PUT");
export const useTestDataForSeo = () => useGroupMutation<void>("/settings/dataforseo/test", "POST");
export const useRemoveDataForSeo = () => useGroupMutation<void>("/settings/dataforseo", "DELETE");
export const useSavePageSpeed = () => useGroupMutation<{ api_key: string }>("/settings/pagespeed", "PUT");
export const useRemovePageSpeed = () => useGroupMutation<void>("/settings/pagespeed", "DELETE");
export const useSaveCrawler = () => useGroupMutation<{ contact: string }>("/settings/crawler", "PUT");
export const useRemoveCrawler = () => useGroupMutation<void>("/settings/crawler", "DELETE");

/** Paid and metered calls by day for the last `days` days. */
export function useUsage(days = 30) {
	return useQuery({
		queryKey: usageKey(days),
		queryFn: async () => (await api<UsageRow[]>(`/usage${query({ days })}`)).data,
	});
}
