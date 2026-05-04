import { describe, expect, it } from "vitest";

import {
  currentProjectInfoReducer,
  resetProjectServerSnapshot,
  selectHasActiveProject,
  selectHasProjectSnapshot,
  setCurrentProjectInfo,
} from "../features/Chat/currentProject";
import type { RootState } from "../app/store";

function rootStateWithCurrentProject(
  currentProject: RootState["current_project"],
): RootState {
  return {
    current_project: currentProject,
    config: {},
    chat: {
      current_thread_id: "",
      threads: {},
    },
  } as RootState;
}

describe("current project state", () => {
  it("preserves server snapshot readiness when a local project update keeps the same workspace", () => {
    const serverSnapshotState = currentProjectInfoReducer(
      undefined,
      setCurrentProjectInfo({
        name: "refact",
        workspaceRoots: ["/workspace/refact"],
        serverSnapshotReceived: true,
      }),
    );

    const localUpdateState = currentProjectInfoReducer(
      serverSnapshotState,
      setCurrentProjectInfo({
        name: "refact",
      }),
    );

    expect(localUpdateState).toEqual({
      name: "refact",
      workspaceRoots: ["/workspace/refact"],
      serverSnapshotReceived: true,
    });
  });

  it("resets server snapshot readiness when local workspace identity changes", () => {
    const serverSnapshotState = currentProjectInfoReducer(
      undefined,
      setCurrentProjectInfo({
        name: "refact",
        workspaceRoots: ["/workspace/refact"],
        serverSnapshotReceived: true,
      }),
    );

    const localUpdateState = currentProjectInfoReducer(
      serverSnapshotState,
      setCurrentProjectInfo({
        name: "other-project",
        workspaceRoots: ["/workspace/other-project"],
      }),
    );

    expect(localUpdateState).toEqual({
      name: "other-project",
      workspaceRoots: ["/workspace/other-project"],
      serverSnapshotReceived: false,
    });
  });

  it("resets server snapshot readiness explicitly", () => {
    const serverSnapshotState = currentProjectInfoReducer(
      undefined,
      setCurrentProjectInfo({
        name: "refact",
        workspaceRoots: ["/workspace/refact"],
        serverSnapshotReceived: true,
      }),
    );

    const resetState = currentProjectInfoReducer(
      serverSnapshotState,
      resetProjectServerSnapshot(),
    );

    expect(resetState.serverSnapshotReceived).toBe(false);
  });

  it("treats an explicit empty server workspace as a received snapshot", () => {
    const state = currentProjectInfoReducer(
      undefined,
      setCurrentProjectInfo({
        name: "",
        workspaceRoots: [],
        serverSnapshotReceived: true,
      }),
    );
    const rootState = rootStateWithCurrentProject(state);

    expect(selectHasProjectSnapshot(rootState)).toBe(true);
    expect(selectHasActiveProject(rootState)).toBe(false);
  });
});
