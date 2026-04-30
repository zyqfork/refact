import { createApi, fetchBaseQuery } from "@reduxjs/toolkit/query/react";
import type { RootState } from "../../app/store";

export type WorktreeLifecycleState =
  | "active"
  | "stale"
  | "deleted"
  | "missing"
  | "conflicted";

export type WorktreeMeta = {
  id: string;
  kind: string;
  root: string;
  source_workspace_root: string;
  repo_root: string;
  branch?: string | null;
  base_branch?: string | null;
  base_commit?: string | null;
  task_id?: string | null;
  card_id?: string | null;
  agent_id?: string | null;
  enforce: boolean;
  reference_count?: number;
  referencing_chat_ids?: string[];
  affected_chat_ids?: string[];
  lifecycle_state?: WorktreeLifecycleState;
  stale?: boolean;
  deleted?: boolean;
  status?: WorktreeStatus | null;
};

export type WorktreeReference = {
  kind: string;
  chat_id?: string | null;
  task_id?: string | null;
  card_id?: string | null;
  agent_id?: string | null;
};

export type WorktreeStatus = {
  path_exists: boolean;
  is_git_worktree: boolean;
  dirty: boolean;
  staged_count: number;
  unstaged_count: number;
  untracked_count: number;
  branch?: string | null;
  head_commit?: string | null;
  error?: string | null;
  lifecycle_state?: WorktreeLifecycleState;
  stale?: boolean;
  deleted?: boolean;
  conflicted?: boolean;
};

export type WorktreeRecordView = {
  meta: WorktreeMeta;
  created_at: string;
  updated_at: string;
  last_seen_at?: string | null;
  references: WorktreeReference[];
  reference_count: number;
  referencing_chat_ids?: string[];
  status: WorktreeStatus;
};

export type WorktreeListResponse = {
  project_hash: string;
  source_workspace_root: string;
  worktrees: WorktreeRecordView[];
};

export type CreateWorktreeRequest = {
  source_workspace_root?: string;
  branch?: string;
  base_branch?: string;
  chat_id?: string;
  kind?: string;
  task_id?: string;
  card_id?: string;
  agent_id?: string;
};

export type CreateWorktreeResponse = {
  worktree: WorktreeRecordView;
  branch_was_created: boolean;
  dirty_source_warning: boolean;
  warnings: string[];
};

export type GetWorktreeRequest = {
  id: string;
  source_workspace_root?: string;
};

export type WorktreeDiffFile = {
  path: string;
  status: string;
  source: string;
};

export type WorktreeDiffStats = {
  committed_files: number;
  staged_files: number;
  unstaged_files: number;
  untracked_files: number;
  files_changed: number;
};

export type WorktreeDiffResponse = {
  id: string;
  branch?: string | null;
  base_branch?: string | null;
  base_commit?: string | null;
  files: WorktreeDiffFile[];
  stats: WorktreeDiffStats;
  patch: string;
  patch_truncated: boolean;
};

export type GetWorktreeDiffRequest = GetWorktreeRequest & {
  max_patch_bytes?: number;
};

export type MergeWorktreeRequest = {
  id: string;
  source_workspace_root?: string;
  target_branch?: string;
  squash?: boolean;
  delete_after_merge?: boolean;
  delete_branch?: boolean;
};

export type MergeWorktreeResponse = {
  merged?: boolean;
  success?: boolean;
  message?: string;
  has_conflicts?: boolean;
  conflicted?: boolean;
  conflict_files?: string[];
  worktree?: WorktreeRecordView | null;
  deleted?: boolean;
  branch_deleted?: boolean;
  affected_references?: WorktreeReference[];
  affected_reference_count?: number;
  affected_chat_ids?: string[];
  warnings?: string[];
};

export type DeleteWorktreeRequest = {
  id: string;
  source_workspace_root?: string;
  delete_branch?: boolean;
};

export type DeleteWorktreeResponse = {
  deleted: boolean;
  branch_deleted: boolean;
  stale_path: boolean;
  affected_references: WorktreeReference[];
  affected_reference_count: number;
  affected_chat_ids?: string[];
  warnings: string[];
};

export type OpenWorktreeRequest = GetWorktreeRequest;

export type OpenWorktreeResponse = {
  id: string;
  path: string;
  branch?: string | null;
  can_open_folder: boolean;
};

type WorktreeQueryParams = Record<
  string,
  string | number | boolean | null | undefined
>;

function buildWorktreeUrl(
  port: number,
  path: string,
  query?: WorktreeQueryParams,
): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== undefined && value !== null) {
      params.set(key, String(value));
    }
  }
  const queryString = params.toString();
  return `http://127.0.0.1:${port}/v1${path}${
    queryString ? `?${queryString}` : ""
  }`;
}

export const worktreesApi = createApi({
  reducerPath: "worktreesApi",
  baseQuery: fetchBaseQuery({
    prepareHeaders: (headers, { getState }) => {
      const token = (getState() as RootState).config.apiKey;
      if (token) {
        headers.set("Authorization", `Bearer ${token}`);
      }
      return headers;
    },
  }),
  tagTypes: ["Worktrees"],
  endpoints: (builder) => ({
    listWorktrees: builder.query<
      WorktreeListResponse,
      { source_workspace_root?: string } | void
    >({
      queryFn: async (args, api, _opts, baseQuery) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(port, "/worktrees", {
            source_workspace_root: args?.source_workspace_root,
          }),
        });
        if (result.error) return { error: result.error };
        return { data: result.data as WorktreeListResponse };
      },
      providesTags: (result) =>
        result
          ? [
              { type: "Worktrees", id: "LIST" },
              ...result.worktrees.map((worktree) => ({
                type: "Worktrees" as const,
                id: worktree.meta.id,
              })),
            ]
          : [{ type: "Worktrees", id: "LIST" }],
    }),
    createWorktree: builder.mutation<
      CreateWorktreeResponse,
      CreateWorktreeRequest
    >({
      queryFn: async (args, api, _opts, baseQuery) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(port, "/worktrees"),
          method: "POST",
          body: args,
        });
        if (result.error) return { error: result.error };
        return { data: result.data as CreateWorktreeResponse };
      },
      invalidatesTags: ["Worktrees"],
    }),
    getWorktree: builder.query<WorktreeRecordView, GetWorktreeRequest>({
      queryFn: async ({ id, source_workspace_root }, api, _opts, baseQuery) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(port, `/worktrees/${encodeURIComponent(id)}`, {
            source_workspace_root,
          }),
        });
        if (result.error) return { error: result.error };
        return { data: result.data as WorktreeRecordView };
      },
      providesTags: (_result, _error, { id }) => [{ type: "Worktrees", id }],
    }),
    getWorktreeDiff: builder.query<
      WorktreeDiffResponse,
      GetWorktreeDiffRequest
    >({
      queryFn: async (
        { id, source_workspace_root, max_patch_bytes },
        api,
        _opts,
        baseQuery,
      ) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(
            port,
            `/worktrees/${encodeURIComponent(id)}/diff`,
            {
              source_workspace_root,
              max_patch_bytes,
            },
          ),
        });
        if (result.error) return { error: result.error };
        return { data: result.data as WorktreeDiffResponse };
      },
      providesTags: (_result, _error, { id }) => [{ type: "Worktrees", id }],
    }),
    mergeWorktree: builder.mutation<
      MergeWorktreeResponse,
      MergeWorktreeRequest
    >({
      queryFn: async (
        { id, source_workspace_root, ...body },
        api,
        _opts,
        baseQuery,
      ) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(
            port,
            `/worktrees/${encodeURIComponent(id)}/merge`,
            {
              source_workspace_root,
            },
          ),
          method: "POST",
          body,
        });
        if (result.error) return { error: result.error };
        return { data: result.data as MergeWorktreeResponse };
      },
      invalidatesTags: (_result, _error, { id }) => [
        { type: "Worktrees", id },
        { type: "Worktrees", id: "LIST" },
      ],
    }),
    deleteWorktree: builder.mutation<
      DeleteWorktreeResponse,
      DeleteWorktreeRequest
    >({
      queryFn: async (
        { id, source_workspace_root, delete_branch },
        api,
        _opts,
        baseQuery,
      ) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(port, `/worktrees/${encodeURIComponent(id)}`, {
            source_workspace_root,
            delete_branch,
          }),
          method: "DELETE",
        });
        if (result.error) return { error: result.error };
        return { data: result.data as DeleteWorktreeResponse };
      },
      invalidatesTags: (_result, _error, { id }) => [
        { type: "Worktrees", id },
        { type: "Worktrees", id: "LIST" },
      ],
    }),
    openWorktree: builder.mutation<OpenWorktreeResponse, OpenWorktreeRequest>({
      queryFn: async ({ id, source_workspace_root }, api, _opts, baseQuery) => {
        const state = api.getState() as RootState;
        const port = state.config.lspPort;
        const result = await baseQuery({
          url: buildWorktreeUrl(
            port,
            `/worktrees/${encodeURIComponent(id)}/open`,
            {
              source_workspace_root,
            },
          ),
          method: "POST",
        });
        if (result.error) return { error: result.error };
        return { data: result.data as OpenWorktreeResponse };
      },
    }),
  }),
});

export const {
  useListWorktreesQuery,
  useCreateWorktreeMutation,
  useGetWorktreeQuery,
  useGetWorktreeDiffQuery,
  useMergeWorktreeMutation,
  useDeleteWorktreeMutation,
  useOpenWorktreeMutation,
} = worktreesApi;
