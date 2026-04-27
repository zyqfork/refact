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
  selectBuddyDiagnostics,
} from "./buddySlice";
import { executeBuddyAction } from "./executeBuddyAction";
import type { BuddyControl } from "./types";
import { PALETTES, STAGES, SIGNALS } from "./constants";
import { computeXpFill } from "./buddyUtils";
import styles from "./BuddyPanel.module.css";

const PANEL_CARE_ACTIONS: BuddyControl[] = [
  { id: "feed", label: "Feed", action: "care_feed", style: "primary" },
  {
    id: "play",
    label: "Play",
    action: "care_play",
    action_param: "bug",
    style: "primary",
  },
  { id: "pet", label: "Pet", action: "care_pet", style: "secondary" },
];

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

  const progression = snapshot?.state.progression;
  const identity = snapshot?.state.identity;
  const pet = snapshot?.state.pet;

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

  const handleCare = useCallback(
    async (ctrl: BuddyControl) => {
      await executeBuddyAction(ctrl, dispatch);
    },
    [dispatch],
  );

  // activeSpeech takes priority; fall back to nowPlaying status text
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

          <div className={styles.needsMini}>
            <span>🍜 {pet?.needs.hunger ?? 0}</span>
            <span>⚡ {pet?.needs.energy ?? 0}</span>
            <span>💕 {pet?.needs.affection ?? 0}</span>
          </div>

          <div className={styles.careRow} onClick={(e) => e.stopPropagation()}>
            {PANEL_CARE_ACTIONS.map((ctrl) => (
              <button
                key={ctrl.id}
                type="button"
                className={styles.careButton}
                onClick={() => void handleCare(ctrl)}
              >
                {ctrl.label}
              </button>
            ))}
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
