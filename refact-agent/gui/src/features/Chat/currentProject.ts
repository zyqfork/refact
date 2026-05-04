import { createReducer, createAction } from "@reduxjs/toolkit";
import { RootState } from "../../app/store";

export type CurrentProjectInfo = {
  name: string;
  workspaceRoots?: string[];
  serverSnapshotReceived: boolean;
};

export type CurrentProjectInfoUpdate = Partial<CurrentProjectInfo>;

type WorkspaceIdentity = Pick<CurrentProjectInfo, "name" | "workspaceRoots">;

const initialState: CurrentProjectInfo = {
  name: "",
  serverSnapshotReceived: false,
};

export const setCurrentProjectInfo = createAction<CurrentProjectInfoUpdate>(
  "currentProjectInfo/setCurrentProjectInfo",
);

export const resetProjectServerSnapshot = createAction(
  "currentProjectInfo/resetProjectServerSnapshot",
);

function workspaceRootsEqual(a?: string[], b?: string[]): boolean {
  if (a === b) return true;
  if (a === undefined || b === undefined) return a === b;
  if (a.length !== b.length) return false;
  return a.every((root, index) => root === b[index]);
}

function hasWorkspaceIdentityChanged(
  state: WorkspaceIdentity,
  update: CurrentProjectInfoUpdate,
): boolean {
  const nameChanged = update.name !== undefined && update.name !== state.name;
  const rootsChanged =
    update.workspaceRoots !== undefined &&
    !workspaceRootsEqual(update.workspaceRoots, state.workspaceRoots);
  return nameChanged || rootsChanged;
}

export const currentProjectInfoReducer = createReducer(
  initialState,
  (builder) => {
    builder
      .addCase(setCurrentProjectInfo, (state, action) => {
        const identityChanged = hasWorkspaceIdentityChanged(
          state,
          action.payload,
        );
        const nextServerSnapshotReceived =
          action.payload.serverSnapshotReceived ??
          (identityChanged ? false : state.serverSnapshotReceived);

        return {
          ...state,
          ...action.payload,
          serverSnapshotReceived: nextServerSnapshotReceived,
        };
      })
      .addCase(resetProjectServerSnapshot, (state) => {
        state.serverSnapshotReceived = false;
      });
  },
);

export const selectThreadProjectOrCurrentProject = (state: RootState) => {
  const threadId = state.chat.current_thread_id;
  const runtime = threadId ? state.chat.threads[threadId] : undefined;
  if (!runtime) {
    return state.current_project.name;
  }
  const thread = runtime.thread;
  if (thread.integration?.project) {
    return thread.integration.project;
  }
  return thread.project_name ?? state.current_project.name;
};

export const selectHasActiveProject = (state: RootState): boolean => {
  const workspaceRoots = state.current_project.workspaceRoots;
  const hasWorkspaceRoot =
    workspaceRoots !== undefined && workspaceRoots.length > 0;
  return Boolean(
    hasWorkspaceRoot ||
      state.current_project.name.trim() ||
      state.config.currentWorkspaceName?.trim(),
  );
};

export const selectHasProjectSnapshot = (state: RootState): boolean =>
  state.current_project.serverSnapshotReceived;
