/** Opts in to React Router 7 behaviour now, so upgrading later changes nothing and tests stay quiet. */
export const routerFuture = { v7_startTransition: true, v7_relativeSplatPath: true } as const;
