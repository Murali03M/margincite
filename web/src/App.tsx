import { Route, Routes } from "react-router-dom";
import { RequireSession } from "./auth/RequireSession";
import { Shell } from "./ui/Shell";
import { NotFound } from "./ui/NotFound";
import { AddSitePage } from "./sites/AddSitePage";
import { AiPage } from "./ai/AiPage";
import { AnswerPage } from "./ai/AnswerPage";
import { BacklinksPage } from "./backlinks/BacklinksPage";
import { ChecksPage } from "./checks/ChecksPage";
import { ConnectionsPage } from "./connections/ConnectionsPage";
import { KeywordsPage } from "./keywords/KeywordsPage";
import { PageDetail } from "./pages/PageDetail";
import { PagesPage } from "./pages/PagesPage";
import { RankingsPage } from "./rankings/RankingsPage";
import { SearchPage } from "./search/SearchPage";
import { SettingsPage as AppSettingsPage } from "./settings/SettingsPage";
import { SettingsPage } from "./sites/SettingsPage";
import { SuggestionPage } from "./suggestions/SuggestionPage";
import { SuggestionsPage } from "./suggestions/SuggestionsPage";
import { SiteOverview } from "./sites/SiteOverview";
import { SitePage } from "./sites/SitePage";
import { SitesPage } from "./sites/SitesPage";

/** Every route of the app. Add a screen by adding a <Route> here. */
export function App() {
	return (
		<RequireSession>
			<Shell>
				<Routes>
					<Route path="/" element={<SitesPage />} />
					<Route path="/sites/new" element={<AddSitePage />} />
					<Route path="/sites/:id" element={<SitePage />}>
						<Route index element={<SiteOverview />} />
						<Route path="suggestions" element={<SuggestionsPage />} />
						<Route path="checks" element={<ChecksPage />} />
						<Route path="keywords" element={<KeywordsPage />} />
						<Route path="rankings" element={<RankingsPage />} />
						<Route path="search" element={<SearchPage />} />
						<Route path="backlinks" element={<BacklinksPage />} />
						<Route path="connections" element={<ConnectionsPage />} />
						<Route path="ai" element={<AiPage />} />
						<Route path="ai/answers/:answerId" element={<AnswerPage />} />
						<Route path="pages" element={<PagesPage />} />
						<Route path="settings" element={<SettingsPage />} />
					</Route>
					<Route path="/settings" element={<AppSettingsPage />} />
					<Route path="/suggestions/:id" element={<SuggestionPage />} />
					<Route path="/pages/:id" element={<PageDetail />} />
					<Route path="*" element={<NotFound />} />
				</Routes>
			</Shell>
		</RequireSession>
	);
}
