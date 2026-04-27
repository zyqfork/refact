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
import { openBuddyChat, newBuddyChatAction } from "../Chat/Thread";
import { push } from "../Pages/pagesSlice";
import {
  useDismissBuddySuggestionMutation,
  useCreateBuddyConversationMutation,
} from "../../services/refact/buddy";
import { useBuddyState } from "./hooks/useBuddyState";
import { BuddyCanvas } from "./BuddyCanvas";
import type { BuddyControl, BuddySuggestion } from "./types";
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
}

export const BuddyChatCompanion: React.FC<Props> = ({ chatId }) => {
  const dispatch = useAppDispatch();
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const runtimeQueue = useAppSelector(selectRuntimeQueue);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const diagnostics = useAppSelector(selectBuddyDiagnostics);
  const suggestions = useAppSelector(selectBuddySuggestions);
  const threadError = useAppSelector((state) =>
    selectChatErrorById(state, chatId),
  );

  const buddy = useBuddyState();
  const [createConversation] = useCreateBuddyConversationMutation();
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
        label: "Ask Buddy",
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
        label: "Fix it →",
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

  const notification: NotificationItem | null = useMemo(() => {
    const normalizedThreadError = threadError?.trim() || null;
    if (normalizedThreadError) {
      return {
        id: `thread-${chatId}`,
        text: normalizedThreadError.slice(0, 160),
        source: "thread",
        controls: errorControls,
        timestamp: Date.now(),
      };
    }

    const runtimeError =
      nowPlaying?.chat_id === chatId && nowPlaying?.status === "failed"
        ? nowPlaying
        : runtimeQueue.find(
            (e) => e.chat_id === chatId && e.status === "failed",
          ) ?? null;
    if (runtimeError) {
      return {
        id: runtimeError.id,
        text: runtimeError.title,
        source: "runtime",
        controls: runtimeError.controls?.length
          ? runtimeError.controls
          : errorControls,
        timestamp: new Date(runtimeError.created_at).getTime(),
      };
    }

    const chatDiagnostic = diagnostics.find((d) => d.chat_id === chatId);
    if (chatDiagnostic?.error_message?.trim()) {
      return {
        id: `diag-${chatId}-${chatDiagnostic.collected_at}`,
        text: chatDiagnostic.error_message.slice(0, 120),
        source: "diagnostic",
        controls: errorControls,
        timestamp: new Date(chatDiagnostic.collected_at).getTime(),
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
      };
    }

    return null;
  }, [
    threadError,
    chatId,
    nowPlaying,
    runtimeQueue,
    diagnostics,
    suggestions,
    errorControls,
    suggestionControls,
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

      if (ctrl.action === "investigate_error") {
        if (pending) return;
        setPending(true);
        try {
          if (notification.source === "suggestion") {
            await dismissMutation(notification.id);
            dispatch(dismissBuddySuggestion(notification.id));
          }
          const result = await createConversation(undefined);
          if ("data" in result && result.data) {
            const meta = result.data;
            dispatch(newBuddyChatAction({ chat_id: meta.chat_id }));
            dispatch(
              openBuddyChat({ chat_id: meta.chat_id, title: meta.title }),
            );
            dispatch(push({ name: "chat" }));
          }
          setDismissedIds((prev) => new Set(prev).add(notification.id));
        } finally {
          setPending(false);
        }
      }
    },
    [notification, pending, createConversation, dismissMutation, dispatch],
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
