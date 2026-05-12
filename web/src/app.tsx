import { QueryClientProvider } from "@tanstack/react-query";

import { queryClient } from "@/lib/query-client";
import { AppRouter } from "@/router";

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppRouter />
    </QueryClientProvider>
  );
}
