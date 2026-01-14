import { useEffect, useRef, useCallback, useMemo } from "react";
import { useAppDispatch } from "./useAppDispatch";
import { useAppSelector } from "./useAppSelector";
import {
  applyChatEvent,
  clearSseRefreshRequest,
} from "../features/Chat/Thread/actions";
import {
  selectOpenThreadIds,
  selectCurrentThreadId,
  selectSseRefreshRequested,
} from "../features/Chat/Thread/selectors";
import { selectLspPort, selectApiKey } from "../features/Config/configSlice";
import { subscribeToChatEvents } from "../services/refact/chatSubscription";

export function useAllChatsSubscription() {
  const dispatch = useAppDispatch();
  const port = useAppSelector(selectLspPort);
  const apiKey = useAppSelector(selectApiKey);
  const openThreadIds = useAppSelector(selectOpenThreadIds);
  const currentThreadId = useAppSelector(selectCurrentThreadId);
  const sseRefreshRequested = useAppSelector(selectSseRefreshRequested);

  const subscriptionsRef = useRef<Map<string, () => void>>(new Map());
  const seqMapRef = useRef<Map<string, bigint>>(new Map());
  const manualCloseRef = useRef<Set<string>>(new Set());
  const desiredIdsRef = useRef<Set<string>>(new Set());
  const portRef = useRef(port);
  const apiKeyRef = useRef(apiKey);
  const subscribeRef = useRef<((chatId: string) => void) | null>(null);

  const allChatIds = useMemo(() => {
    const ids = new Set(openThreadIds);
    if (currentThreadId) {
      ids.add(currentThreadId);
    }
    return Array.from(ids);
  }, [openThreadIds, currentThreadId]);

  const scheduleResubscribe = useCallback((chatId: string, delay: number) => {
    setTimeout(() => {
      if (!desiredIdsRef.current.has(chatId)) return;
      if (subscriptionsRef.current.has(chatId)) return;
      subscribeRef.current?.(chatId);
    }, delay);
  }, []);

  const subscribeToChat = useCallback(
    (chatId: string) => {
      if (subscriptionsRef.current.has(chatId)) return;
      if (!portRef.current) return;
      if (!desiredIdsRef.current.has(chatId)) return;

      manualCloseRef.current.delete(chatId);
      seqMapRef.current.set(chatId, 0n);

      const unsubscribe = subscribeToChatEvents(
        chatId,
        portRef.current,
        {
          onEvent: (envelope) => {
            const seq = BigInt(envelope.seq);
            const lastSeq = seqMapRef.current.get(chatId) ?? 0n;

            if (envelope.type === "snapshot") {
              seqMapRef.current.set(chatId, seq);
            } else {
              if (seq <= lastSeq) return;
              if (seq > lastSeq + 1n) {
                const unsub = subscriptionsRef.current.get(chatId);
                if (unsub) {
                  manualCloseRef.current.add(chatId);
                  unsub();
                  subscriptionsRef.current.delete(chatId);
                }
                scheduleResubscribe(chatId, 100);
                return;
              }
              seqMapRef.current.set(chatId, seq);
            }
            dispatch(applyChatEvent(envelope));
          },
          onError: () => {
            subscriptionsRef.current.delete(chatId);
            if (!manualCloseRef.current.has(chatId)) {
              scheduleResubscribe(chatId, 2000);
            }
          },
          onDisconnected: () => {
            subscriptionsRef.current.delete(chatId);
            if (!manualCloseRef.current.has(chatId)) {
              scheduleResubscribe(chatId, 2000);
            }
          },
        },
        apiKeyRef.current ?? undefined,
      );

      subscriptionsRef.current.set(chatId, unsubscribe);
    },
    [dispatch, scheduleResubscribe],
  );

  subscribeRef.current = subscribeToChat;
  const subscribe = subscribeToChat;

  const unsubscribe = useCallback((chatId: string) => {
    manualCloseRef.current.add(chatId);
    const unsub = subscriptionsRef.current.get(chatId);
    if (unsub) {
      unsub();
      subscriptionsRef.current.delete(chatId);
      seqMapRef.current.delete(chatId);
    }
  }, []);

  const unsubscribeAll = useCallback(() => {
    for (const chatId of subscriptionsRef.current.keys()) {
      manualCloseRef.current.add(chatId);
    }
    for (const unsub of subscriptionsRef.current.values()) {
      unsub();
    }
    subscriptionsRef.current.clear();
    seqMapRef.current.clear();
  }, []);

  useEffect(() => {
    if (port !== portRef.current || apiKey !== apiKeyRef.current) {
      unsubscribeAll();
      portRef.current = port;
      apiKeyRef.current = apiKey;
    }

    if (!port) return;

    const currentIds = new Set(allChatIds);
    desiredIdsRef.current = currentIds;
    const subscribedIds = new Set(subscriptionsRef.current.keys());

    for (const id of currentIds) {
      if (!subscribedIds.has(id)) {
        subscribe(id);
      }
    }

    for (const id of subscribedIds) {
      if (!currentIds.has(id)) {
        unsubscribe(id);
      }
    }
  }, [allChatIds, port, apiKey, subscribe, unsubscribe, unsubscribeAll]);

  useEffect(() => {
    if (sseRefreshRequested) {
      dispatch(clearSseRefreshRequest());
      const chatId = sseRefreshRequested;
      unsubscribe(chatId);
      setTimeout(() => subscribe(chatId), 50);
    }
  }, [sseRefreshRequested, dispatch, subscribe, unsubscribe]);

  useEffect(() => {
    return () => {
      unsubscribeAll();
    };
  }, [unsubscribeAll]);
}
