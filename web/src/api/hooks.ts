import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, query } from "./client";
import type {
	AiAnswer,
	AiAnswerDetail,
	AiDomain,
	AiPrompt,
	AiRun,
	AiSettings,
	BacklinkSource,
	BacklinkTarget,
	BingStatus,
	ChecksList,
	Crawl,
	CreatedSite,
	Finding,
	GoogleProperty,
	GoogleStatus,
	Integrations,
	ItemStatus,
	Keyword,
	Me,
	Meta,
	Metrics,
	PageDetail,
	PageSummary,
	SearchPageRow,
	SearchQuery,
	Site,
	SiteSummary,
	SuggestionDetail,
	SuggestionOut,
} from "./types";

/** Query keys in one place, so screens can refresh each other's data. */
export const keys = {
	me: ["me"] as const,
	sites: ["sites"] as const,
	site: (id: string) => ["site", id] as const,
	summary: (id: string) => ["summary", id] as const,
	crawl: (id: string) => ["crawl", id] as const,
	suggestions: (siteId: string) => ["suggestions", siteId] as const,
	suggestion: (id: string) => ["suggestion", id] as const,
	pages: (crawlId: string) => ["pages", crawlId] as const,
	page: (id: string) => ["page", id] as const,
	findings: (pageId: string) => ["findings", pageId] as const,
	integrations: ["integrations"] as const,
	metrics: (siteId: string) => ["metrics", siteId] as const,
	checks: (siteId: string) => ["checks", siteId] as const,
	keywords: (siteId: string) => ["keywords", siteId] as const,
	google: (siteId: string) => ["google", siteId] as const,
	bing: (siteId: string) => ["bing", siteId] as const,
	search: (siteId: string) => ["search", siteId] as const,
	backlinks: (siteId: string) => ["backlinks", siteId] as const,
	aiSettings: (siteId: string) => ["ai-settings", siteId] as const,
	aiPrompts: (siteId: string) => ["ai-prompts", siteId] as const,
	aiRuns: (siteId: string) => ["ai-runs", siteId] as const,
	aiRun: (runId: string) => ["ai-run", runId] as const,
	aiAnswers: (runId: string) => ["ai-answers", runId] as const,
	aiAnswer: (id: string) => ["ai-answer", id] as const,
	aiDomains: (siteId: string) => ["ai-domains", siteId] as const,
};

/** A page of a list, with the server's paging figures. */
export type Paged<T> = { items: T[]; meta: Meta };

const EMPTY_META: Meta = { total: 0, page: 1, limit: 20 };

async function fetchPaged<T>(path: string): Promise<Paged<T>> {
	const { data, meta } = await api<T[]>(path);
	return { items: data, meta: meta ?? { ...EMPTY_META, total: data.length } };
}

/** How often a running crawl is asked for its progress. */
export const CRAWL_POLL_MS = 2000;

export const isCrawlActive = (crawl: Crawl | undefined): boolean =>
	crawl?.status === "queued" || crawl?.status === "running";

export async function fetchSites(): Promise<Site[]> {
	return (await api<Site[]>("/sites")).data;
}

/** The signed-in person. */
export function useMe() {
	return useQuery({ queryKey: keys.me, queryFn: async () => (await api<Me>("/me")).data });
}

export function useSites() {
	return useQuery({ queryKey: keys.sites, queryFn: fetchSites });
}

export function useSite(id: string) {
	return useQuery({ queryKey: keys.site(id), queryFn: async () => (await api<Site>(`/sites/${id}`)).data });
}

/** Score, counts, top suggestions and crawl facts for one site. */
export function useSummary(id: string) {
	return useQuery({
		queryKey: keys.summary(id),
		queryFn: async () => (await api<SiteSummary>(`/sites/${id}/summary`)).data,
	});
}

/**
 * One crawl. While it is queued or running it is asked again every two seconds, and it stops when it ends.
 * Pass `poll: false` to only read what another component already fetches.
 */
export function useCrawl(id: string | null | undefined, { poll = true }: { poll?: boolean } = {}) {
	return useQuery({
		queryKey: keys.crawl(id ?? ""),
		queryFn: async () => (await api<Crawl>(`/crawls/${id}`)).data,
		enabled: Boolean(id),
		refetchInterval: poll ? (query) => (isCrawlActive(query.state.data) ? CRAWL_POLL_MS : false) : false,
	});
}

export function useCreateSite() {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: { url: string; name?: string }) =>
			(await api<CreatedSite>("/sites", { method: "POST", body: input })).data,
		onSuccess: () => client.invalidateQueries({ queryKey: keys.sites }),
	});
}

/** "Crawl again". Rejects with the server's own 409 and 429 messages. */
export function useStartCrawl(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<Crawl>(`/sites/${siteId}/crawls`, { method: "POST" })).data,
		onSuccess: () => client.invalidateQueries({ queryKey: keys.summary(siteId) }),
	});
}

export type SuggestionFilters = { status: string; category: string; page: number };

/** The ranked suggestions of a site, filtered and paged by the server. */
export function useSuggestions(siteId: string, { status, category, page }: SuggestionFilters) {
	return useQuery({
		queryKey: [...keys.suggestions(siteId), status, category, page],
		queryFn: () =>
			fetchPaged<SuggestionOut>(`/sites/${siteId}/suggestions${query({ status, category, page })}`),
	});
}

/** Items per request on the suggestion screen. */
export const ITEM_LIMIT = 100;

export function useSuggestion(id: string, page: number) {
	return useQuery({
		queryKey: [...keys.suggestion(id), page],
		queryFn: async () => {
			const { data, meta } = await api<SuggestionDetail>(`/suggestions/${id}${query({ page, limit: ITEM_LIMIT })}`);
			return { detail: data, meta: meta ?? { ...EMPTY_META, limit: ITEM_LIMIT, total: data.items.length } };
		},
	});
}

export type StatusChange = { status: ItemStatus; targets?: string[] };

type Cached = { detail: SuggestionDetail; meta: Meta };

/** The detail with `change` applied: the matching items and the three counts move together. */
export function applyChange(detail: SuggestionDetail, change: StatusChange): SuggestionDetail {
	const hit = (target: string) => !change.targets || change.targets.includes(target);
	const moved = detail.items.filter((item) => hit(item.target) && item.status !== change.status);
	const count = (status: ItemStatus) => moved.filter((item) => item.status === status).length;
	const s = detail.suggestion;
	const gained = moved.length;
	return {
		...detail,
		items: detail.items.map((item) => (hit(item.target) ? { ...item, status: change.status } : item)),
		suggestion: {
			...s,
			open: s.open - count("open") + (change.status === "open" ? gained : 0),
			done: s.done - count("done") + (change.status === "done" ? gained : 0),
			dismissed: s.dismissed - count("dismissed") + (change.status === "dismissed" ? gained : 0),
		},
	};
}

/**
 * Done, Dismiss and Reopen. The screen changes at once; if the server refuses, the previous
 * data is put back and the mutation's error carries the server's message.
 */
export function useChangeStatus(id: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (change: StatusChange) =>
			(await api<{ changed: number }>(`/suggestions/${id}`, { method: "PATCH", body: change })).data,
		onMutate: async (change) => {
			await client.cancelQueries({ queryKey: keys.suggestion(id) });
			const before = client.getQueriesData<Cached>({ queryKey: keys.suggestion(id) });
			for (const [key, cached] of before) {
				if (cached) client.setQueryData<Cached>(key, { ...cached, detail: applyChange(cached.detail, change) });
			}
			return { before };
		},
		onError: (_error, _change, context) => {
			for (const [key, cached] of context?.before ?? []) client.setQueryData(key, cached);
		},
		onSettled: () => {
			void client.invalidateQueries({ queryKey: keys.suggestion(id) });
			void client.invalidateQueries({ queryKey: ["suggestions"] });
			void client.invalidateQueries({ queryKey: ["summary"] });
		},
	});
}

/** The pages of one crawl. Pass null to wait until the crawl is known. */
export function usePages(crawlId: string | null, page: number) {
	return useQuery({
		queryKey: [...keys.pages(crawlId ?? ""), page],
		queryFn: () => fetchPaged<PageSummary>(`/crawls/${crawlId}/pages${query({ page })}`),
		enabled: Boolean(crawlId),
	});
}

export function usePage(id: string) {
	return useQuery({ queryKey: keys.page(id), queryFn: async () => (await api<PageDetail>(`/pages/${id}`)).data });
}

export function usePageFindings(id: string) {
	return useQuery({
		queryKey: keys.findings(id),
		queryFn: async () => (await api<Finding[]>(`/pages/${id}/findings`)).data,
	});
}

/** Every suggestion of a site, to link a finding to its suggestion. Waits for a site id. */
export function useAllSuggestions(siteId: string | null | undefined) {
	return useQuery({
		queryKey: [...keys.suggestions(siteId ?? ""), "all"],
		queryFn: async () => (await api<SuggestionOut[]>(`/sites/${siteId}/suggestions?status=all&limit=100`)).data,
		enabled: Boolean(siteId),
	});
}

export function useDeleteSite(id: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<{ deleted: boolean }>(`/sites/${id}`, { method: "DELETE" })).data,
		onSuccess: () => client.invalidateQueries({ queryKey: keys.sites }),
	});
}

/* ---- Metrics, checks and integrations status ---- */

/** What the server is set up with: the secret key, Google credentials and AI engine keys. */
export function useIntegrations() {
	return useQuery({ queryKey: keys.integrations, queryFn: async () => (await api<Integrations>("/integrations")).data });
}

/** The seven numbers of the project card. */
export function useMetrics(siteId: string) {
	return useQuery({
		queryKey: keys.metrics(siteId),
		queryFn: async () => (await api<Metrics>(`/sites/${siteId}/metrics`)).data,
	});
}

export function useChecks(siteId: string) {
	return useQuery({
		queryKey: keys.checks(siteId),
		queryFn: async () => (await api<ChecksList>(`/sites/${siteId}/checks`)).data,
	});
}

/** Refreshes everything that is computed from keywords, Search Console, Bing or AI runs. */
function refreshNumbers(client: ReturnType<typeof useQueryClient>, siteId: string) {
	for (const key of [keys.metrics(siteId), keys.checks(siteId), keys.summary(siteId), keys.keywords(siteId)]) {
		void client.invalidateQueries({ queryKey: key });
	}
}

/* ---- Keywords ---- */

export function useKeywords(siteId: string, enabled = true) {
	return useQuery({
		queryKey: keys.keywords(siteId),
		queryFn: async () => (await api<Keyword[]>(`/sites/${siteId}/keywords`)).data,
		enabled,
	});
}

export function useAddKeyword(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: { keyword: string; target_url?: string }) =>
			(await api<Keyword>(`/sites/${siteId}/keywords`, { method: "POST", body: input })).data,
		onSuccess: () => refreshNumbers(client, siteId),
	});
}

export function useSetKeywordTarget(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: { id: string; target_url: string | null }) =>
			(await api<Keyword>(`/keywords/${input.id}`, { method: "PATCH", body: { target_url: input.target_url } })).data,
		onSuccess: () => refreshNumbers(client, siteId),
	});
}

export function useDeleteKeyword(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (id: string) => (await api<{ deleted: boolean }>(`/keywords/${id}`, { method: "DELETE" })).data,
		onSuccess: () => refreshNumbers(client, siteId),
	});
}

/* ---- Google Search Console ---- */

export function useGoogle(siteId: string) {
	return useQuery({
		queryKey: keys.google(siteId),
		queryFn: async () => (await api<GoogleStatus>(`/sites/${siteId}/integrations/google`)).data,
	});
}

/** The properties the person can choose from. Pass `enabled: false` until the chooser is shown. */
export function useGoogleProperties(siteId: string, enabled: boolean) {
	return useQuery({
		queryKey: [...keys.google(siteId), "properties"],
		queryFn: async () => (await api<GoogleProperty[]>(`/sites/${siteId}/integrations/google/properties`)).data,
		enabled,
	});
}

/** Asks for the Google sign-in address. The browser must be sent there afterwards. */
export function useStartGoogle(siteId: string) {
	return useMutation({
		mutationFn: async () =>
			(await api<{ auth_url: string }>(`/sites/${siteId}/integrations/google/start`, { method: "POST" })).data,
	});
}

export function useChooseProperty(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (property: string) =>
			(await api<GoogleStatus>(`/sites/${siteId}/integrations/google`, { method: "PUT", body: { property } })).data,
		onSuccess: (data) => {
			client.setQueryData(keys.google(siteId), data);
			refreshNumbers(client, siteId);
		},
	});
}

export function useSyncGoogle(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () =>
			(await api<GoogleStatus>(`/sites/${siteId}/integrations/google/sync`, { method: "POST" })).data,
		onSuccess: (data) => client.setQueryData(keys.google(siteId), data),
	});
}

export function useDisconnectGoogle(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<GoogleStatus>(`/sites/${siteId}/integrations/google`, { method: "DELETE" })).data,
		onSuccess: (data) => {
			client.setQueryData(keys.google(siteId), data);
			void client.invalidateQueries({ queryKey: keys.search(siteId) });
			refreshNumbers(client, siteId);
		},
	});
}

/* ---- Bing Webmaster Tools ---- */

export function useBing(siteId: string) {
	return useQuery({
		queryKey: keys.bing(siteId),
		queryFn: async () => (await api<BingStatus>(`/sites/${siteId}/integrations/bing`)).data,
	});
}

/** Saves the key. The key is only sent in the request body and is never kept anywhere. */
export function useSaveBing(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: { api_key: string; site_url: string }) =>
			(await api<BingStatus>(`/sites/${siteId}/integrations/bing`, { method: "PUT", body: input })).data,
		onSuccess: (data) => {
			client.setQueryData(keys.bing(siteId), data);
			void client.invalidateQueries({ queryKey: keys.backlinks(siteId) });
			refreshNumbers(client, siteId);
		},
	});
}

export function useSyncBing(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<BingStatus>(`/sites/${siteId}/integrations/bing/sync`, { method: "POST" })).data,
		onSuccess: (data) => client.setQueryData(keys.bing(siteId), data),
	});
}

export function useDisconnectBing(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<BingStatus>(`/sites/${siteId}/integrations/bing`, { method: "DELETE" })).data,
		onSuccess: (data) => {
			client.setQueryData(keys.bing(siteId), data);
			void client.invalidateQueries({ queryKey: keys.backlinks(siteId) });
			refreshNumbers(client, siteId);
		},
	});
}

/* ---- Search Console data and Bing backlinks ---- */

export type QuerySort = "clicks" | "impressions" | "position";

export function useSearchQueries(siteId: string, page: number, sort: QuerySort, enabled: boolean) {
	return useQuery({
		queryKey: [...keys.search(siteId), "queries", page, sort],
		queryFn: () => fetchPaged<SearchQuery>(`/sites/${siteId}/search/queries${query({ page, sort })}`),
		enabled,
	});
}

export function useSearchPages(siteId: string, page: number, enabled: boolean) {
	return useQuery({
		queryKey: [...keys.search(siteId), "pages", page],
		queryFn: () => fetchPaged<SearchPageRow>(`/sites/${siteId}/search/pages${query({ page })}`),
		enabled,
	});
}

export function useBacklinks(siteId: string, page: number, enabled: boolean) {
	return useQuery({
		queryKey: [...keys.backlinks(siteId), "targets", page],
		queryFn: () => fetchPaged<BacklinkTarget>(`/sites/${siteId}/backlinks${query({ page })}`),
		enabled,
	});
}

/** The pages that link to one target. Waits until a target is chosen. */
export function useBacklinkSources(siteId: string, target: string | null) {
	return useQuery({
		queryKey: [...keys.backlinks(siteId), "sources", target],
		queryFn: async () =>
			(await api<BacklinkSource[]>(`/sites/${siteId}/backlinks/sources${query({ target_url: target ?? "" })}`)).data,
		enabled: Boolean(target),
	});
}

/* ---- AI visibility ---- */

export function useAiSettings(siteId: string) {
	return useQuery({
		queryKey: keys.aiSettings(siteId),
		queryFn: async () => (await api<AiSettings>(`/sites/${siteId}/ai/settings`)).data,
	});
}

export function useSaveAiSettings(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (input: AiSettings) =>
			(await api<AiSettings>(`/sites/${siteId}/ai/settings`, { method: "PUT", body: input })).data,
		onSuccess: (data) => client.setQueryData(keys.aiSettings(siteId), data),
	});
}

export function useAiPrompts(siteId: string) {
	return useQuery({
		queryKey: keys.aiPrompts(siteId),
		queryFn: async () => (await api<AiPrompt[]>(`/sites/${siteId}/ai/prompts`)).data,
	});
}

export function useAddPrompt(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (text: string) =>
			(await api<AiPrompt>(`/sites/${siteId}/ai/prompts`, { method: "POST", body: { text } })).data,
		onSuccess: () => void client.invalidateQueries({ queryKey: keys.aiPrompts(siteId) }),
	});
}

export function useDeletePrompt(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async (id: string) => (await api<{ deleted: boolean }>(`/ai/prompts/${id}`, { method: "DELETE" })).data,
		onSuccess: () => {
			void client.invalidateQueries({ queryKey: keys.aiPrompts(siteId) });
			void client.invalidateQueries({ queryKey: keys.checks(siteId) });
		},
	});
}

/** Ten questions written by Claude. Nothing is saved until the person adds one. */
export function useSuggestPrompts(siteId: string) {
	return useMutation({
		mutationFn: async () =>
			(await api<{ suggestions: string[] }>(`/sites/${siteId}/ai/prompts/suggest`, { method: "POST" })).data.suggestions,
	});
}

/** How often a running AI check is asked for its progress. */
export const AI_RUN_POLL_MS = 3000;

export const isRunActive = (run: AiRun | undefined): boolean => run?.status === "queued" || run?.status === "running";

/** The newest runs of a site, newest first. */
export function useAiRuns(siteId: string) {
	return useQuery({
		queryKey: keys.aiRuns(siteId),
		queryFn: () => fetchPaged<AiRun>(`/sites/${siteId}/ai/runs${query({ limit: 10 })}`),
	});
}

/** One run, asked again every three seconds while it is queued or running. */
export function useAiRun(runId: string | null) {
	return useQuery({
		queryKey: keys.aiRun(runId ?? ""),
		queryFn: async () => (await api<AiRun>(`/ai/runs/${runId}`)).data,
		enabled: Boolean(runId),
		refetchInterval: (q) => (isRunActive(q.state.data) ? AI_RUN_POLL_MS : false),
	});
}

export function useStartRun(siteId: string) {
	const client = useQueryClient();
	return useMutation({
		mutationFn: async () => (await api<AiRun>(`/sites/${siteId}/ai/runs`, { method: "POST" })).data,
		onSuccess: () => void client.invalidateQueries({ queryKey: keys.aiRuns(siteId) }),
	});
}

/** Called once when a run has ended, so the grid, domains, card and checks show its results. */
export function useRefreshAfterRun(siteId: string) {
	const client = useQueryClient();
	return () => {
		for (const key of [keys.aiRuns(siteId), keys.aiDomains(siteId), keys.metrics(siteId), keys.checks(siteId)]) {
			void client.invalidateQueries({ queryKey: key });
		}
		void client.invalidateQueries({ queryKey: ["ai-answers"] });
	};
}

export function useRunAnswers(runId: string | null) {
	return useQuery({
		queryKey: keys.aiAnswers(runId ?? ""),
		queryFn: async () => (await api<AiAnswer[]>(`/ai/runs/${runId}/answers`)).data,
		enabled: Boolean(runId),
	});
}

export function useAiAnswer(id: string) {
	return useQuery({
		queryKey: keys.aiAnswer(id),
		queryFn: async () => (await api<AiAnswerDetail>(`/ai/answers/${id}`)).data,
	});
}

export function useAiDomains(siteId: string) {
	return useQuery({
		queryKey: keys.aiDomains(siteId),
		queryFn: async () => (await api<AiDomain[]>(`/sites/${siteId}/ai/domains`)).data,
	});
}
