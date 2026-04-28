import { render, waitFor } from "../../utils/test-utils";
import { describe, expect, it } from "vitest";
import {
  server,
  goodUser,
  goodPing,
  chatLinks,
  goodCaps,
  emptyTrajectories,
  trajectorySave,
  trajectoryDelete,
  chatSessionSubscribe,
  chatSessionCommand,
  chatSessionAbort,
  sidebarSubscribe,
  emptyTasks,
} from "../../utils/mockServer";
import { InnerApp } from "../../features/App";
import { HistoryState } from "../../features/History/historySlice";

describe("Delete a Chat form history", () => {
  it("can delete a chat", async () => {
    server.use(
      goodUser,
      goodPing,
      chatLinks,
      goodCaps,
      emptyTrajectories,
      trajectorySave,
      trajectoryDelete,
      chatSessionSubscribe,
      chatSessionCommand,
      chatSessionAbort,
      sidebarSubscribe,
      emptyTasks,
    );
    const now = new Date().toISOString();
    const history: HistoryState = {
      chats: {
        abc123: {
          title: "Test title",
          isTitleGenerated: false,
          messages: [],
          id: "abc123",
          model: "foo",
          tool_use: "quick",
          new_chat_suggested: {
            wasSuggested: false,
          },
          createdAt: now,
          updatedAt: now,
        },
      },
      isLoading: false,
      loadError: null,
      pagination: { cursor: null, hasMore: false },
    };
    const { user, store, ...app } = render(<InnerApp />, {
      preloadedState: {
        history,
        pages: [{ name: "history" }],
        config: {
          apiKey: "test",
          lspPort: 8001,
          themeProps: {},
          host: "vscode",
        },
      },
    });

    const itemTitleToDelete = "Test title";

    const restoreButtonText = await app.findByText(itemTitleToDelete);

    // Find the delete button - uses aria-label="Delete chat"
    let container = restoreButtonText.parentElement;
    while (
      container &&
      !container.querySelector('[aria-label="Delete chat"]')
    ) {
      container = container.parentElement;
    }
    const deleteButton = container?.querySelector('[aria-label="Delete chat"]');

    expect(deleteButton).not.toBeNull();

    // eslint-disable-next-line @typescript-eslint/no-non-null-assertion
    await user.click(deleteButton!);

    // Wait for the deletion to be processed
    await waitFor(() => {
      expect(store.getState().history.chats).toEqual({});
    });
  });
});
