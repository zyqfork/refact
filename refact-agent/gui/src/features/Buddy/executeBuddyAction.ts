import type { AppDispatch } from "../../app/store";
import { push } from "../Pages/pagesSlice";
import { clearActiveSpeech, setBuddySnapshot } from "./buddySlice";
import {
  openChatInModeAndStart,
  switchToThread,
  startBuddyInvestigation,
} from "../Chat/Thread";
import { isValidSetupMode } from "../Setup/setupModes";
import type { BuddyControl } from "./types";
import type { DiagnosticContext } from "./types";
import { buddyApi } from "../../services/refact/buddy";

/**
 * Central executor for all Buddy control actions.
 *
 * Every Buddy surface (BuddyHome, BuddyPanel, BuddySpeechCloud,
 * NavigationRequest handler) must route through this single function
 * so that action semantics are defined in exactly one place.
 */
export async function executeBuddyAction(
  ctrl: BuddyControl,
  dispatch: AppDispatch,
  investigation?: {
    triggerText: string;
    triggerSource:
      | "thread"
      | "runtime"
      | "diagnostic"
      | "suggestion"
      | "frontend";
    sourceChatId?: string;
    diagnostic?: DiagnosticContext | null;
  },
): Promise<void> {
  switch (ctrl.action) {
    case "dismiss":
      dispatch(clearActiveSpeech());
      break;

    case "open_setup":
      void dispatch(openChatInModeAndStart({ mode: "setup" }));
      dispatch(clearActiveSpeech());
      break;

    case "open_setup_mode": {
      const param = ctrl.action_param ?? "";
      const mode = isValidSetupMode(param) ? param : "setup";
      void dispatch(openChatInModeAndStart({ mode }));
      dispatch(clearActiveSpeech());
      break;
    }

    case "open_stats":
      dispatch(push({ name: "stats dashboard" }));
      dispatch(clearActiveSpeech());
      break;

    case "open_buddy":
      dispatch(push({ name: "buddy" }));
      dispatch(clearActiveSpeech());
      break;

    case "investigate_error": {
      dispatch(clearActiveSpeech());
      if (!investigation) break;
      await dispatch(startBuddyInvestigation(investigation));
      break;
    }

    case "care_feed":
    case "care_play":
    case "care_pet":
    case "care_sleep":
    case "care_clean": {
      const action = ctrl.action.replace("care_", "") as
        | "feed"
        | "play"
        | "pet"
        | "sleep"
        | "clean";
      const result = await dispatch(
        buddyApi.endpoints.careBuddy.initiate({
          action,
          toy: ctrl.action_param,
        }),
      ).unwrap();
      dispatch(setBuddySnapshot(result.snapshot));
      break;
    }

    case "reroll_personality": {
      const result = await dispatch(
        buddyApi.endpoints.rerollBuddyPersonality.initiate(undefined),
      ).unwrap();
      dispatch(setBuddySnapshot(result.snapshot));
      break;
    }

    default:
      dispatch(clearActiveSpeech());
  }
}

/**
 * Central executor for engine-driven NavigationRequest events.
 *
 * Maps engine view names to actual GUI page dispatches.
 * Every NavigationRequest from the sidebar SSE must route through here.
 */
export function executeBuddyNavigation(
  view: string,
  params: Record<string, unknown> | undefined,
  dispatch: AppDispatch,
): void {
  switch (view) {
    case "buddy":
    case "buddy_home":
      dispatch(push({ name: "buddy" }));
      break;

    case "stats":
    case "dashboard":
      dispatch(push({ name: "stats dashboard" }));
      break;

    case "chat": {
      const chatId =
        typeof params?.chat_id === "string" ? params.chat_id : undefined;
      if (chatId) {
        dispatch(switchToThread({ id: chatId }));
      }
      dispatch(push({ name: "chat" }));
      break;
    }

    case "setup": {
      const mode = typeof params?.mode === "string" ? params.mode : "setup";
      const validMode = isValidSetupMode(mode) ? mode : "setup";
      void dispatch(openChatInModeAndStart({ mode: validMode }));
      break;
    }

    case "settings":
    case "customization":
      dispatch(push({ name: "customization" }));
      break;

    case "knowledge":
      dispatch(push({ name: "knowledge graph" }));
      break;

    case "tasks":
      dispatch(push({ name: "tasks list" }));
      break;

    case "integrations":
      dispatch(push({ name: "integrations page" }));
      break;
  }
}
