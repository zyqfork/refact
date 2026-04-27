import React, { useCallback, useMemo } from "react";
import { useAppDispatch, useAppSelector } from "../../hooks";
import { push } from "../Pages/pagesSlice";
import { BuddyCanvas } from "./BuddyCanvas";
import { useBuddyState } from "./hooks/useBuddyState";
import {
  selectBuddySnapshot,
  selectIsBuddyEnabled,
  selectNowPlaying,
  selectActiveSpeech,
  clearActiveSpeech,
} from "./buddySlice";
import { isValidSetupMode } from "../Setup/setupModes";
import {
  openBuddyChat,
  newBuddyChatAction,
  openChatInModeAndStart,
} from "../Chat/Thread";
import { useCreateBuddyConversationMutation } from "../../services/refact/buddy";
import type { BuddyControl } from "./types";
import { PALETTES, STAGES, SIGNALS } from "./constants";
import { computeXpFill } from "./buddyUtils";
import styles from "./BuddyPanel.module.css";

export const BuddyPanel: React.FC = () => {
  const dispatch = useAppDispatch();
  const snapshot = useAppSelector(selectBuddySnapshot);
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const activeSpeech = useAppSelector(selectActiveSpeech);
  const [createConversation] = useCreateBuddyConversationMutation();

  const buddy = useBuddyState();
  const { state } = buddy;

  const handleSpeechControl = useCallback(
    async (ctrl: BuddyControl) => {
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
          const result = await createConversation(undefined);
          if ("data" in result && result.data) {
            const meta = result.data;
            dispatch(newBuddyChatAction({ chat_id: meta.chat_id }));
            dispatch(
              openBuddyChat({ chat_id: meta.chat_id, title: meta.title }),
            );
            dispatch(push({ name: "chat" }));
          }
          break;
        }
        default:
          dispatch(clearActiveSpeech());
      }
    },
    [dispatch, createConversation],
  );

  const paletteIndex =
    snapshot?.state.identity.palette_index ?? state.paletteIndex;
  const palette = PALETTES[paletteIndex] ?? PALETTES[0];

  const progression = snapshot?.state.progression;
  const identity = snapshot?.state.identity;

  const stageIdx = progression?.stage ?? state.progress.stage;
  const stage = STAGES[stageIdx] ?? STAGES[0];

  const xp = progression?.xp ?? state.progress.xp;

  const xpFill = useMemo(
    () => computeXpFill(progression?.xp ?? 0, progression?.xp_next ?? 100),
    [progression],
  );

  const name = identity?.name ?? state.name;

  const handleOpen = useCallback(() => {
    dispatch(push({ name: "buddy" }));
  }, [dispatch]);

  // activeSpeech takes priority; fall back to nowPlaying status text
  const speechText = activeSpeech
    ? activeSpeech.text
    : nowPlaying?.speech_text ?? nowPlaying?.title ?? null;
  const speechControls = activeSpeech ? activeSpeech.controls : undefined;
  const speechHandler = activeSpeech ? handleSpeechControl : undefined;

  if (snapshot === null) return null;
  if (!enabled) return null;

  return (
    <div
      className={styles.block}
      onClick={handleOpen}
      style={{ cursor: "pointer" }}
    >
      <div className={styles.body}>
        <div className={styles.scene}>
          {/* Stop propagation so bubble action buttons don't also open the Buddy page */}
          <div className={styles.glowWrap} onClick={(e) => e.stopPropagation()}>
            <div
              className={styles.glow}
              style={{ backgroundColor: palette.body }}
            />
            <BuddyCanvas
              state={state}
              onEvent={buddy.handleCanvasEvent}
              displaySize={200}
              speechOverride={speechText}
              speechControls={speechControls}
              onSpeechControlClick={speechHandler}
            />
          </div>
        </div>

        <div className={styles.info}>
          <div className={styles.nameRow}>
            <span className={styles.name}>{name}</span>
            <span
              className={styles.stageBadge}
              style={{
                backgroundColor: palette.body + "33",
                color: palette.body,
              }}
            >
              {stage.emoji} {stage.name}
            </span>
            <div className={styles.xpBarInline}>
              <div
                className={styles.xpFillInline}
                style={{ width: `${xpFill}%` }}
              />
            </div>
            <span className={styles.xpText}>{xp}</span>
          </div>

          {nowPlaying && nowPlaying.progress != null && (
            <div className={styles.statusBubble}>
              <span className={styles.statusIcon}>
                {SIGNALS[nowPlaying.signal_type]?.icon ?? "⚡"}
              </span>
              <div className={styles.progressBar}>
                <div style={{ width: `${nowPlaying.progress}%` }} />
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
