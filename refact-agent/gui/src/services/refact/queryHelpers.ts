import type { FetchArgs, FetchBaseQueryError } from "@reduxjs/toolkit/query";
import type { RootState } from "../../app/store";

type InnerBaseQuery = (
  arg: string | FetchArgs,
) => Promise<
  | { data: unknown; error?: undefined }
  | { error: FetchBaseQueryError; data?: undefined }
>;

export function lspQueryFn<TArg, TResult>(
  buildRequest: (arg: TArg, port: number) => string | FetchArgs,
) {
  return async (
    arg: TArg,
    api: { getState: () => unknown },
    _opts: object,
    baseQuery: InnerBaseQuery,
  ) => {
    const state = api.getState() as RootState;
    const port = state.config.lspPort;
    if (!port) {
      return {
        error: {
          status: 500,
          data: "Missing lspPort in config",
        } as FetchBaseQueryError,
      };
    }
    const request = buildRequest(arg, port);
    const result = await baseQuery(
      typeof request === "string" ? { url: request } : request,
    );
    if (result.error) {
      return {
        error: {
          status: result.error.status as number,
          data: result.error.data ? String(result.error.data) : "Unknown error",
        } as FetchBaseQueryError,
      };
    }
    return { data: result.data as TResult };
  };
}
