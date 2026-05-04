import { describe, expect, it, vi } from "vitest";

import { render, screen, stubResizeObserver } from "../utils/test-utils";
import { Dashboard } from "../features/Dashboard";

vi.mock("../features/Buddy/BuddyDashboardScene", () => ({
  BuddyDashboardScene: () => null,
}));

vi.mock("../features/Dashboard/components/ChatsSection/ChatsSection", () => ({
  ChatsSection: ({ projectLoading }: { projectLoading: boolean }) => (
    <div data-testid="chats-section">
      CHATS {projectLoading ? "Loading" : "Loaded"}
    </div>
  ),
}));

vi.mock("../features/Dashboard/components/TasksSection/TasksSection", () => ({
  TasksSection: ({ projectLoading }: { projectLoading: boolean }) => (
    <div data-testid="tasks-section">
      TASKS {projectLoading ? "Loading" : "Loaded"}
    </div>
  ),
}));

const ONLINE_CONNECTION_STATE = {
  browserOnline: true,
  backendStatus: "online" as const,
  backendLastOkAt: 1,
  backendError: null,
  sseConnections: {},
};

describe("Dashboard project readiness", () => {
  it("shows chats and tasks loading until a server project snapshot is received", () => {
    stubResizeObserver();
    render(<Dashboard />, {
      preloadedState: {
        connection: ONLINE_CONNECTION_STATE,
        current_project: {
          name: "refact",
          workspaceRoots: ["/workspace/refact"],
          serverSnapshotReceived: false,
        },
      },
    });

    expect(screen.getByTestId("chats-section")).toHaveTextContent(
      "CHATS Loading",
    );
    expect(screen.getByTestId("tasks-section")).toHaveTextContent(
      "TASKS Loading",
    );
  });

  it("stops chats and tasks loading after an explicit empty server workspace snapshot", () => {
    stubResizeObserver();
    render(<Dashboard />, {
      preloadedState: {
        connection: ONLINE_CONNECTION_STATE,
        current_project: {
          name: "",
          workspaceRoots: [],
          serverSnapshotReceived: true,
        },
      },
    });

    expect(screen.getByTestId("chats-section")).toHaveTextContent(
      "CHATS Loaded",
    );
    expect(screen.getByTestId("tasks-section")).toHaveTextContent(
      "TASKS Loaded",
    );
  });
});
