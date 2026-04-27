import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useAppDispatch, useAppSelector } from "../../hooks";
import {
  selectNowPlaying,
  selectBuddyDiagnostics,
  selectIsBuddyEnabled,
  selectRuntimeQueue,
  selectBuddySuggestions,
  dismissBuddySuggestion,
} from "./buddySlice";
import { selectChatErrorById } from "../Chat/Thread";
import { startBuddyInvestigation } from "../Chat/Thread";
import { push } from "../Pages/pagesSlice";
import { useDismissBuddySuggestionMutation } from "../../services/refact/buddy";
import { useBuddyState } from "./hooks/useBuddyState";
import { BuddyCanvas } from "./BuddyCanvas";
import type { BuddyControl, BuddySuggestion, DiagnosticContext } from "./types";
import { isBuddyOverlaySuppressedIssue } from "./investigation";
import { executeBuddyAction } from "./executeBuddyAction";
import { selectBuddySnapshot } from "./buddySlice";
import styles from "./BuddyChatCompanion.module.css";

interface Props {
  chatId: string;
}

interface NotificationItem {
  id: string;
  text: string;
  source: "thread" | "runtime" | "diagnostic" | "suggestion";
  controls: BuddyControl[];
  timestamp: number;
  diagnostic?: DiagnosticContext | null;
}

export const BuddyChatCompanion: React.FC<Props> = ({ chatId }) => {
  const dispatch = useAppDispatch();
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const runtimeQueue = useAppSelector(selectRuntimeQueue);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const diagnostics = useAppSelector(selectBuddyDiagnostics);
  const suggestions = useAppSelector(selectBuddySuggestions);
  const snapshot = useAppSelector(selectBuddySnapshot);
  const threadError = useAppSelector((state) =>
    selectChatErrorById(state, chatId),
  );

  const buddy = useBuddyState();
  const [dismissMutation] = useDismissBuddySuggestionMutation();

  const [dismissedIds, setDismissedIds] = useState<Set<string>>(new Set());
  const [pending, setPending] = useState(false);
  const prevChatIdRef = useRef(chatId);

  useEffect(() => {
    if (prevChatIdRef.current !== chatId) {
      prevChatIdRef.current = chatId;
      setDismissedIds(new Set());
    }
  }, [chatId]);

  const errorControls: BuddyControl[] = useMemo(
    () => [
      {
        id: "ask",
        label: "Investigate",
        action: "investigate_error",
        style: "primary",
      },
      {
        id: "dismiss",
        label: "Dismiss",
        action: "dismiss",
        style: "ghost",
      },
    ],
    [],
  );

  const suggestionControls: BuddyControl[] = useMemo(
    () => [
      {
        id: "fix",
        label: "Investigate",
        action: "investigate_error",
        style: "primary",
      },
      {
        id: "ignore",
        label: "Ignore",
        action: "dismiss",
        style: "ghost",
      },
    ],
    [],
  );

  const careControls: BuddyControl[] = useMemo(
    () => [
      { id: "feed", label: "Feed", action: "care_feed", style: "primary" },
      {
        id: "play",
        label: "Play",
        action: "care_play",
        action_param: "bug",
        style: "secondary",
      },
      { id: "pet", label: "Pet", action: "care_pet", style: "secondary" },
    ],
    [],
  );

  const notification: NotificationItem | null = useMemo(() => {
    const chatDiagnostic =
      diagnostics.find((d) => d.chat_id === chatId) ?? null;
    const normalizedThreadError = threadError?.trim() || null;
    if (normalizedThreadError) {
      if (
        isBuddyOverlaySuppressedIssue(normalizedThreadError, chatDiagnostic)
      ) {
        return null;
      }
      return {
        id: `thread-${chatId}`,
        text: normalizedThreadError.slice(0, 160),
        source: "thread",
        controls: errorControls,
        timestamp: Date.now(),
        diagnostic: chatDiagnostic,
      };
    }

    const runtimeError =
      nowPlaying?.chat_id === chatId && nowPlaying?.status === "failed"
        ? nowPlaying
        : runtimeQueue.find(
            (e) => e.chat_id === chatId && e.status === "failed",
          ) ?? null;
    if (runtimeError) {
      if (isBuddyOverlaySuppressedIssue(runtimeError.title, chatDiagnostic)) {
        return null;
      }
      return {
        id: runtimeError.id,
        text: runtimeError.title,
        source: "runtime",
        controls: runtimeError.controls?.length
          ? runtimeError.controls
          : errorControls,
        timestamp: new Date(runtimeError.created_at).getTime(),
        diagnostic: chatDiagnostic,
      };
    }

    if (chatDiagnostic?.error_message?.trim()) {
      if (
        isBuddyOverlaySuppressedIssue(
          chatDiagnostic.error_message,
          chatDiagnostic,
        )
      ) {
        return null;
      }
      return {
        id: `diag-${chatId}-${chatDiagnostic.collected_at}`,
        text: chatDiagnostic.error_message.slice(0, 120),
        source: "diagnostic",
        controls: errorControls,
        timestamp: new Date(chatDiagnostic.collected_at).getTime(),
        diagnostic: chatDiagnostic,
      };
    }

    const activeSuggestion = suggestions.find(
      (s: BuddySuggestion) => !s.dismissed,
    );
    if (activeSuggestion) {
      return {
        id: activeSuggestion.id,
        text: `${activeSuggestion.title}: ${activeSuggestion.description}`,
        source: "suggestion",
        controls: suggestionControls,
        timestamp: new Date(activeSuggestion.created_at).getTime(),
        diagnostic: null,
      };
    }

    const needsCare =
      (snapshot?.state?.pet?.condition?.hungry ?? false) ||
      (snapshot?.state?.pet?.condition?.bored ?? false) ||
      (snapshot?.state?.pet?.condition?.lonely ?? false);
    if (!needsCare) {
      return null;
    }

    return {
      id: `care-${chatId}`,
      text: "Need a quick check-in? Feed, play, or pet me.",
      source: "runtime",
      controls: careControls,
      timestamp: Date.now(),
      diagnostic: null,
    };
  }, [
    threadError,
    chatId,
    nowPlaying,
    runtimeQueue,
    diagnostics,
    suggestions,
    snapshot,
    errorControls,
    suggestionControls,
    careControls,
  ]);

  const isDismissed = notification ? dismissedIds.has(notification.id) : false;

  useEffect(() => {
    if (!notification || isDismissed) return;
    const t = setTimeout(() => {
      setDismissedIds((prev) => new Set(prev).add(notification.id));
    }, 15000);
    return () => clearTimeout(t);
  }, [notification, isDismissed]);

  const handleControl = useCallback(
    async (ctrl: BuddyControl) => {
      if (!notification) return;

      if (ctrl.action === "dismiss" || ctrl.action === "dismiss_speech") {
        if (notification.source === "suggestion") {
          await dismissMutation(notification.id);
          dispatch(dismissBuddySuggestion(notification.id));
        }
        setDismissedIds((prev) => new Set(prev).add(notification.id));
        return;
      }

      if (ctrl.action === "open_buddy") {
        setDismissedIds((prev) => new Set(prev).add(notification.id));
        dispatch(push({ name: "buddy" }));
        return;
      }

      if (ctrl.action.startsWith("care_")) {
        await executeBuddyAction(ctrl, dispatch);
        setDismissedIds((prev) => new Set(prev).add(notification.id));
        return;
      }

      if (ctrl.action === "investigate_error") {
        if (pending) return;
        setPending(true);
        try {
          if (notification.source === "suggestion") {
            await dismissMutation(notification.id);
            dispatch(dismissBuddySuggestion(notification.id));
          }
          await dispatch(
            startBuddyInvestigation({
              triggerText: notification.text,
              triggerSource: notification.source,
              sourceChatId: chatId,
              diagnostic: notification.diagnostic,
            }),
          );
          setDismissedIds((prev) => new Set(prev).add(notification.id));
        } finally {
          setPending(false);
        }
      }
    },
    [notification, pending, dismissMutation, dispatch, chatId],
  );

  if (!enabled || !notification || isDismissed) return null;

  return (
    <div className={styles.companion}>
      <BuddyCanvas
        state={buddy.state}
        onEvent={buddy.handleCanvasEvent}
        displaySize={160}
        speechOverride={notification.text}
        speechControls={notification.controls}
        onSpeechControlClick={handleControl}
        bubblePosition="left"
      />
    </div>
  );
};
