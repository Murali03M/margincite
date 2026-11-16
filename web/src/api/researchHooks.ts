import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "./client";
import { keys } from "./hooks";
import type {
	AiOverviewRow,
	KeywordIdea,
	Rankings,
	SiteSettings,
	SiteSettingsInput,
	SpeedReport,
	VolumeStatus,
} from "./types";

export const researchKeys = {
	siteSettings: (siteId: string) => ["site-settings", siteId] as const,
	rankings: (siteId: string) => ["rankings", siteId] as const,
	overviews: (siteId: string) => ["ai-overviews", siteId] as const,
	speed: (siteId: string) => ["speed", siteId] as const,
	volumes: (siteId: string) => ["keyword-volumes", siteId] as const,
};

/** How often a check that was just started is asked whether it has finished. */
export const CHECK_POLL_MS = 4000;

/* ---- Site options ---- */

export function useSiteSettings(siteId: string) {
	return useQuery({
		queryKey: researchKeys.siteSettings(siteId),
		queryFn: async () => (await api<SiteSettings>(`/sites/${siteId}/settings`)).data,
	});
}

export function useSaveSiteSettings(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: Partial<SiteSettingsInput>) =>
			(await api<SiteSettings>(`/sites/${siteId}/settings`, { method: "PUT", body: input })).data,
		onSuccess: (data) => {
			client.setQueryData(researchKeys.siteSettings(siteId), data);
			void client.invalidateQueries({ queryKey: keys.site(siteId) });
			void client.invalidateQueries({ queryKey: keys.sites });
			void client.invalidateQueries({ queryKey: researchKeys.rankings(siteId) });
		},
	});
}

/* ---- Rankings and AI Overviews ---- */

/** Pass `watching` while a check runs, to ask again every few seconds. */
export function useRankings(siteId: string, enabled: boolean, watching: boolean, pollMs = CHECK_POLL_MS) {
	return useQuery({
		queryKey: researchKeys.rankings(siteId),
		queryFn: async () => (await api<Rankings>(`/sites/${siteId}/rankings`)).data,
		enabled,
		refetchInterval: watching ? pollMs : false,
	});
}

/** Starts a check of every tracked keyword. Rejects with the server's 409, 422 and 429 messages. */
export function useCheckRankings(siteId: string) {
	return useMutation({
		mutationFn: async () => (await api<unknown>(`/sites/${siteId}/rankings/check`, { method: "POST" })).data,
	});
}

export function useAiOverviews(siteId: string, enabled: boolean) {
	return useQuery({
		queryKey: researchKeys.overviews(siteId),
		queryFn: async () => (await api<AiOverviewRow[]>(`/sites/${siteId}/ai-overviews`)).data,
		enabled,
	});
}

/** Everything that is computed from fresh rankings, speed or volumes. */
export function useRefreshComputed(siteId: string) {
	const client = useQueryClient();
	return () => {
		for (const key of [
			keys.metrics(siteId),
			keys.keywords(siteId),
			researchKeys.rankings(siteId),
			researchKeys.overviews(siteId),
			researchKeys.speed(siteId),
		]) {
			void client.invalidateQueries({ queryKey: key });
		}
	};
}

/* ---- Keyword volumes and ideas ---- */

/** True while a volume refresh is waiting or running on the server. */
export const volumesBusy = (status: VolumeStatus | undefined): boolean => status?.status === "queued" || status?.status === "running";

/**
 * Where the last volume refresh stands. While one is in progress it is asked again every few seconds,
 * unless `poll` is false (the caller has stopped watching, or only wants to read what is known).
 */
export function useVolumeStatus(siteId: string, poll = true, pollMs = CHECK_POLL_MS) {
	return useQuery({
		queryKey: researchKeys.volumes(siteId),
		queryFn: async () => (await api<VolumeStatus>(`/sites/${siteId}/keywords/volumes`)).data,
		refetchInterval: (query) => (poll && volumesBusy(query.state.data) ? pollMs : false),
	});
}

/** Asks for fresh volumes. Rejects with the server's 409, 422 and 429 messages. */
export function useRefreshVolumes(siteId: string) {
	return useMutation({
		mutationFn: async () => (await api<unknown>(`/sites/${siteId}/keywords/volumes`, { method: "POST" })).data,
	});
}

/** Up to 50 ideas for a seed. Nothing is saved until the person adds one. */
export function useKeywordIdeas(siteId: string) {
	return useMutation({
		mutationFn: async (seed: string) =>
			(await api<{ ideas: KeywordIdea[] }>(`/sites/${siteId}/keywords/ideas`, { method: "POST", body: { seed } })).data.ideas,
	});
}

/* ---- Page speed ---- */

export function useSpeed(siteId: string, watching: boolean, pollMs = CHECK_POLL_MS) {
	return useQuery({
		queryKey: researchKeys.speed(siteId),
		queryFn: async () => (await api<SpeedReport>(`/sites/${siteId}/speed`)).data,
		refetchInterval: watching ? pollMs : false,
	});
}

export function useCheckSpeed(siteId: string) {
	return useMutation({
		mutationFn: async () => (await api<unknown>(`/sites/${siteId}/speed/check`, { method: "POST" })).data,
	});
}
