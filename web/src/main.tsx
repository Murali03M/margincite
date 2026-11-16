import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { makeQueryClient } from "./api/queryClient";
import { routerFuture } from "./routerFuture";
import "./index.css";

const client = makeQueryClient();

createRoot(document.getElementById("root") as HTMLElement).render(
	<StrictMode>
		<QueryClientProvider client={client}>
			<BrowserRouter future={routerFuture}>
				<App />
			</BrowserRouter>
		</QueryClientProvider>
	</StrictMode>,
);
