import { configureStore } from "@reduxjs/toolkit";
import { afterEach, describe, expect, test, vi } from "vitest";
import {
  worktreesApi,
  type WorktreeDiffResponse,
  type WorktreeRecordView,
} from "./worktrees";

type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

type TestConfigState = {
  lspPort: number;
  apiKey: string | null;
};

function createTestStore(config: TestConfigState) {
  return configureStore({
    reducer: {
      config: (state: TestConfigState = config) => state,
      [worktreesApi.reducerPath]: worktreesApi.reducer,
    },
    middleware: (getDefaultMiddleware) =>
      getDefaultMiddleware().concat(worktreesApi.middleware),
  });
}

function makeWorktreeRecord(id = "wt-1"): WorktreeRecordView {
  return {
    meta: {
      id,
      kind: "chat",
      root: `/tmp/${id}`,
      source_workspace_root: "/repo",
      repo_root: "/repo",
      branch: `refact/${id}`,
      base_branch: "main",
      base_commit: "abc123",
      enforce: false,
    },
    created_at: "2026-04-30T00:00:00Z",
    updated_at: "2026-04-30T00:00:00Z",
    references: [{ kind: "chat", chat_id: "chat-1" }],
    reference_count: 1,
    status: {
      path_exists: true,
      is_git_worktree: true,
      dirty: false,
      staged_count: 0,
      unstaged_count: 0,
      untracked_count: 0,
      branch: `refact/${id}`,
      head_commit: "abc123",
    },
  };
}

function makeDiffResponse(id = "wt-1"): WorktreeDiffResponse {
  return {
    id,
    files: [],
    stats: {
      committed_files: 0,
      staged_files: 0,
      unstaged_files: 0,
      untracked_files: 0,
      files_changed: 0,
    },
    patch: "",
    patch_truncated: false,
  };
}

function jsonResponse(data: unknown): Response {
  return new Response(JSON.stringify(data), {
    headers: { "Content-Type": "application/json" },
  });
}

function firstRequest(fetchMock: ReturnType<typeof vi.fn<FetchLike>>): Request {
  expect(fetchMock).toHaveBeenCalled();
  const [input, init] = fetchMock.mock.calls[0];
  return input instanceof Request ? input : new Request(input, init);
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("Worktrees RTK Query API", () => {
  test("createWorktree uses dynamic port auth and request body", async () => {
    const fetchMock = vi.fn<FetchLike>();
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockResolvedValueOnce(
      jsonResponse({
        worktree: makeWorktreeRecord(),
        branch_was_created: true,
        dirty_source_warning: false,
        warnings: [],
      }),
    );
    const store = createTestStore({ lspPort: 8123, apiKey: "test-token" });

    const request = store.dispatch(
      worktreesApi.endpoints.createWorktree.initiate({
        source_workspace_root: "/repo",
        branch: "feature/worktree",
        chat_id: "chat-1",
        kind: "chat",
      }),
    );
    await request;

    const fetchRequest = firstRequest(fetchMock);
    expect(fetchRequest.url).toBe("http://127.0.0.1:8123/v1/worktrees");
    expect(fetchRequest.method).toBe("POST");
    expect(fetchRequest.headers.get("authorization")).toBe("Bearer test-token");
    await expect(fetchRequest.clone().json()).resolves.toEqual({
      source_workspace_root: "/repo",
      branch: "feature/worktree",
      chat_id: "chat-1",
      kind: "chat",
    });
  });

  test("getWorktreeDiff encodes id and query parameters", async () => {
    const fetchMock = vi.fn<FetchLike>();
    vi.stubGlobal("fetch", fetchMock);
    fetchMock.mockResolvedValueOnce(jsonResponse(makeDiffResponse("wt/1")));
    const store = createTestStore({ lspPort: 8123, apiKey: null });

    const request = store.dispatch(
      worktreesApi.endpoints.getWorktreeDiff.initiate({
        id: "wt/1",
        source_workspace_root: "/repo",
        max_patch_bytes: 4096,
      }),
    );
    await request;
    request.unsubscribe();

    const fetchRequest = firstRequest(fetchMock);
    const url = new URL(fetchRequest.url);
    expect(fetchRequest.method).toBe("GET");
    expect(url.pathname).toBe("/v1/worktrees/wt%2F1/diff");
    expect(url.searchParams.get("source_workspace_root")).toBe("/repo");
    expect(url.searchParams.get("max_patch_bytes")).toBe("4096");
  });
});
