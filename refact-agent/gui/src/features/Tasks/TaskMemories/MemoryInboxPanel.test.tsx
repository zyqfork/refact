import { describe, expect, it } from "vitest";
import { http, HttpResponse } from "msw";
import { render, screen, waitFor } from "../../../utils/test-utils";
import { server } from "../../../utils/mockServer";
import { MemoryInboxPanel } from "./MemoryInboxPanel";
import type { TaskMemoriesResponse } from "../../../services/refact/taskMemoriesApi";

HTMLElement.prototype.hasPointerCapture = () => false;

const CONFIG_STATE = {
  config: {
    apiKey: "test",
    lspPort: 8001,
    themeProps: {},
    host: "web" as const,
  },
};

const memoriesResponse: TaskMemoriesResponse = {
  task_id: "task-1",
  since: "2026-05-22T00:00:00Z",
  new_count: 5,
  warnings: [],
  memories: [
    {
      filename: "decision.md",
      created_at: "2026-05-22T01:00:00Z",
      created_at_known: true,
      title: "Use scoped memory index",
      content: "Keep memory search local to the current task.",
      tags: ["planner", "search"],
      kind: "decision",
      namespace: "task",
      pinned: false,
      status: "active",
    },
    {
      filename: "risk.md",
      created_at: "2026-05-22T02:00:00Z",
      created_at_known: true,
      title: "Archive stale notes",
      content: "Old progress notes can confuse future agents.",
      tags: ["cleanup"],
      kind: "risk",
      namespace: "card:T-2",
      pinned: true,
      status: "active",
    },
  ],
};

function mockMemories(response: TaskMemoriesResponse = memoriesResponse) {
  server.use(
    http.get("http://127.0.0.1:8001/v1/task/:taskId/memories", () =>
      HttpResponse.json(response),
    ),
  );
}

describe("MemoryInboxPanel", () => {
  it("renders memory list with mock data", async () => {
    mockMemories();

    render(<MemoryInboxPanel taskId="task-1" />, {
      preloadedState: CONFIG_STATE,
    });

    expect(await screen.findByText("Use scoped memory index")).toBeInTheDocument();
    expect(screen.getByText("Archive stale notes")).toBeInTheDocument();
    expect(screen.getByText(/5 new since/)).toBeInTheDocument();
    expect(screen.getByText("pinned")).toBeInTheDocument();
  });

  it("pin and archive actions call mutations", async () => {
    const pinRequests: unknown[] = [];
    const archiveRequests: string[] = [];
    mockMemories();
    server.use(
      http.post(
        "http://127.0.0.1:8001/v1/task/:taskId/memories/:filename/pin",
        async ({ request }) => {
          pinRequests.push(await request.json());
          return HttpResponse.json({
            ok: true,
            filename: "decision.md",
            pinned: true,
            changed: true,
          });
        },
      ),
      http.post(
        "http://127.0.0.1:8001/v1/task/:taskId/memories/:filename/archive",
        ({ params }) => {
          archiveRequests.push(String(params.filename));
          return HttpResponse.json({
            ok: true,
            filename: "decision.md",
            archived_filename: "decision.md",
          });
        },
      ),
    );

    const { user } = render(<MemoryInboxPanel taskId="task-1" />, {
      preloadedState: CONFIG_STATE,
    });

    await user.click(await screen.findByRole("button", { name: "Pin" }));
    await waitFor(() => expect(pinRequests).toEqual([{ pinned: true }]));

    await user.click(screen.getAllByRole("button", { name: "Archive" })[0]);
    await waitFor(() => expect(archiveRequests).toEqual(["decision.md"]));
  });

  it("filters by server-backed kind, namespace, and tag chips", async () => {
    const queryStrings: string[] = [];
    mockMemories();
    server.use(
      http.get("http://127.0.0.1:8001/v1/task/:taskId/memories", ({ request }) => {
        queryStrings.push(new URL(request.url).search);
        return HttpResponse.json(memoriesResponse);
      }),
    );

    const { user } = render(<MemoryInboxPanel taskId="task-1" />, {
      preloadedState: CONFIG_STATE,
    });

    await screen.findByText("Use scoped memory index");
    await user.click(screen.getByRole("combobox", { name: "Memory kind filter" }));
    await user.click(await screen.findByRole("option", { name: "risk" }));
    await user.click(
      screen.getByRole("combobox", { name: "Memory namespace filter" }),
    );
    await user.click(await screen.findByRole("option", { name: "card:T-2" }));
    await user.click(screen.getByRole("button", { name: "cleanup" }));

    await waitFor(() => {
      expect(queryStrings.some((query) => query.includes("kind=risk"))).toBe(
        true,
      );
      expect(
        queryStrings.some((query) => query.includes("namespace=card%3AT-2")),
      ).toBe(true);
      expect(screen.queryByText("Use scoped memory index")).not.toBeInTheDocument();
      expect(screen.getByText("Archive stale notes")).toBeInTheDocument();
    });
  });

  it("search filters client-side", async () => {
    mockMemories();

    const { user } = render(<MemoryInboxPanel taskId="task-1" />, {
      preloadedState: CONFIG_STATE,
    });

    await screen.findByText("Use scoped memory index");
    await user.type(screen.getByLabelText("Search memories"), "stale");

    await waitFor(() => {
      expect(screen.queryByText("Use scoped memory index")).not.toBeInTheDocument();
      expect(screen.getByText("Archive stale notes")).toBeInTheDocument();
    });
  });

  it("mark all triaged calls triage mutation", async () => {
    const triageRequests: unknown[] = [];
    mockMemories();
    server.use(
      http.post(
        "http://127.0.0.1:8001/v1/task/:taskId/memories/triage-done",
        async ({ request }) => {
          triageRequests.push(await request.json());
          return HttpResponse.json({
            ok: true,
            cursor: "2026-05-22T03:00:00.000Z",
          });
        },
      ),
    );

    const { user } = render(<MemoryInboxPanel taskId="task-1" />, {
      preloadedState: CONFIG_STATE,
    });

    await user.click(await screen.findByRole("button", { name: "Mark all triaged" }));

    await waitFor(() => {
      expect(triageRequests).toHaveLength(1);
      const request = triageRequests[0] as { cursor?: unknown };
      expect(typeof request.cursor).toBe("string");
    });
  });
});
