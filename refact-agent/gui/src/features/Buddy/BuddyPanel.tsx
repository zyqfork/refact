import React, { useCallback } from "react";
import { useAppDispatch, useAppSelector } from "../../hooks";
import { push } from "../Pages/pagesSlice";
import { BuddyCanvas } from "./BuddyCanvas";
import { useBuddyState } from "./hooks/useBuddyState";
import {
  selectBuddySnapshot,
  selectIsBuddyEnabled,
  selectNowPlaying,
  selectActiveSpeech,
  selectBuddyDiagnostics,
} from "./buddySlice";
import { executeBuddyAction } from "./executeBuddyAction";
import type { BuddyControl } from "./types";
import { PALETTES, SIGNALS } from "./constants";
import styles from "./BuddyPanel.module.css";

export const BuddyPanel: React.FC = () => {
  const dispatch = useAppDispatch();
  const snapshot = useAppSelector(selectBuddySnapshot);
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const activeSpeech = useAppSelector(selectActiveSpeech);
  const diagnostics = useAppSelector(selectBuddyDiagnostics);

  const buddy = useBuddyState();
  const { state } = buddy;

  const activeDiagnostic = activeSpeech?.chat_id
    ? diagnostics.find((diag) => diag.chat_id === activeSpeech.chat_id)
    : undefined;

  const paletteIndex =
    snapshot?.state.identity.palette_index ?? state.paletteIndex;
  const palette = PALETTES[paletteIndex] ?? PALETTES[0];

  const speechText = activeSpeech
    ? activeSpeech.text
    : nowPlaying?.speech_text ?? nowPlaying?.title ?? null;
  const speechControls = activeSpeech ? activeSpeech.controls : undefined;
  const speechHandler = activeSpeech
    ? async (ctrl: BuddyControl) => {
        await executeBuddyAction(ctrl, dispatch, {
          triggerText: activeSpeech.text,
          triggerSource: "runtime",
          sourceChatId: activeSpeech.chat_id,
          diagnostic: activeDiagnostic,
        });
      }
    : undefined;

  const handleOpen = useCallback(() => {
    dispatch(push({ name: "buddy" }));
  }, [dispatch]);

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
