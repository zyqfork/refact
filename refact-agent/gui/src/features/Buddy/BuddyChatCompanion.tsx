import React, { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";
import { Button } from "@radix-ui/themes";
import { useAppDispatch, useAppSelector } from "../../hooks";
import { push } from "../Pages/pagesSlice";
import {
  selectNowPlaying,
  selectBuddyDiagnostics,
  selectIsBuddyEnabled,
  selectBuddySnapshot,
  selectRuntimeQueue,
} from "./buddySlice";
import { BuddyCanvas } from "./BuddyCanvas";
import { createInitialSemanticState, reduceSemanticState } from "./state";
import type { SemanticAction } from "./state";
import type { BuddySemanticState, BuddyEvent } from "./types";
import styles from "./BuddyChatCompanion.module.css";

interface BuddyChatCompanionProps {
  chatId: string;
}

export const BuddyChatCompanion: React.FC<BuddyChatCompanionProps> = ({ chatId }) => {
  const dispatch = useAppDispatch();
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const snapshot = useAppSelector(selectBuddySnapshot);
  const runtimeQueue = useAppSelector(selectRuntimeQueue);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const diagnostics = useAppSelector(selectBuddyDiagnostics);

  const [dismissed, setDismissed] = useState(false);
  const prevChatIdRef = useRef(chatId);

  const [canvasState, canvasDispatch] = useReducer(
    (s: BuddySemanticState, a: SemanticAction) => reduceSemanticState(s, a),
    undefined,
    createInitialSemanticState,
  );

  useEffect(() => {
    if (prevChatIdRef.current !== chatId) {
      prevChatIdRef.current = chatId;
      setDismissed(false);
    }
  }, [chatId]);

  useEffect(() => {
    if (!snapshot) return;
    canvasDispatch({
      kind: "patch",
      patch: {
        name: snapshot.state.identity.name,
        paletteIndex: snapshot.state.identity.palette_index,
      },
    });
  }, [snapshot?.state.identity.name, snapshot?.state.identity.palette_index]);

  const chatError = useMemo(() => {
    if (nowPlaying?.chat_id === chatId && nowPlaying?.status === "failed") {
      return nowPlaying;
    }
    return runtimeQueue.find((e) => e.chat_id === chatId && e.status === "failed") ?? null;
  }, [runtimeQueue, nowPlaying, chatId]);

  const chatDiagnostic = useMemo(
    () => diagnostics.find((d) => d.chat_id === chatId),
    [diagnostics, chatId],
  );

  const message = chatError?.title ?? chatDiagnostic?.error_message?.slice(0, 120) ?? null;

  useEffect(() => {
    if (message) {
      canvasDispatch({ kind: "signal", signalType: "chat_error" });
      setDismissed(false);
    }
  }, [message]);

  useEffect(() => {
    if (!message || dismissed) return;
    const t = setTimeout(() => setDismissed(true), 15000);
    return () => clearTimeout(t);
  }, [message, dismissed]);

  const handleAskBuddy = useCallback(() => {
    dispatch(push({ name: "buddy" }));
  }, [dispatch]);

  const handleCanvasEvent = useCallback((event: BuddyEvent) => {
    if (event.type === "semantic_update") {
      canvasDispatch({ kind: "patch", patch: event.patch });
    }
  }, []);

  if (!enabled || !message || dismissed) return null;

  return (
    <div className={styles.companion}>
      <div className={styles.miniScene}>
        <BuddyCanvas state={canvasState} onEvent={handleCanvasEvent} displaySize={48} />
        <div className={styles.bubble}>
          <p className={styles.bubbleText}>{message}</p>
          <div className={styles.bubbleTail} />
        </div>
      </div>
      <div className={styles.actions}>
        <Button size="1" variant="ghost" onClick={handleAskBuddy}>
          Ask Buddy
        </Button>
        <Button size="1" variant="ghost" color="gray" onClick={() => setDismissed(true)}>
          ×
        </Button>
      </div>
    </div>
  );
};
