import { useMemo } from "react";
import type { ProviderListItem } from "../../../services/refact";

export function useGetConfiguredProvidersView({
  configuredProviders,
}: {
  configuredProviders: ProviderListItem[];
}) {
  const sortedConfiguredProviders = useMemo(() => {
    return [...configuredProviders].sort((a, b) => {
      const getPriority = (provider: { name: string }) => {
        if (provider.name === "custom") return 2;
        return 1;
      };

      const priorityA = getPriority(a);
      const priorityB = getPriority(b);

      if (priorityA !== priorityB) {
        return priorityA - priorityB;
      }

      return a.name.localeCompare(b.name);
    });
  }, [configuredProviders]);

  return {
    sortedConfiguredProviders,
  };
}
