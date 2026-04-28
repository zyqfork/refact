import { providersApi } from "../services/refact";
import { useAppSelector } from "./useAppSelector";
import { selectBackendStatus } from "../features/Connection";
import { selectLspPort } from "../features/Config/configSlice";

export function useGetConfiguredProvidersQuery() {
  const backendStatus = useAppSelector(selectBackendStatus);
  const lspPort = useAppSelector(selectLspPort);
  return providersApi.useGetConfiguredProvidersQuery(lspPort, {
    skip:
      backendStatus === "unknown" || !Number.isFinite(lspPort) || lspPort <= 0,
  });
}

export function useGetProviderQuery({
  providerName,
}: {
  providerName: string;
}) {
  return providersApi.useGetProviderQuery({ providerName });
}

export function useGetProviderSchemaQuery({
  providerName,
}: {
  providerName: string;
}) {
  return providersApi.useGetProviderSchemaQuery({ providerName });
}

export function useGetProviderModelsQuery({
  providerName,
}: {
  providerName: string;
}) {
  return providersApi.useGetProviderModelsQuery({ providerName });
}

export function useUpdateProviderMutation() {
  return providersApi.useUpdateProviderMutation();
}

export function useDeleteProviderMutation() {
  return providersApi.useDeleteProviderMutation();
}

export function useGetDefaultsQuery() {
  return providersApi.useGetDefaultsQuery(undefined);
}

export function useUpdateDefaultsMutation() {
  return providersApi.useUpdateDefaultsMutation();
}
