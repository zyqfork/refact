import React, { useCallback, useMemo, useState } from "react";
import { Flex, Text, Button, Spinner } from "@radix-ui/themes";
import { ArrowLeftIcon, GearIcon } from "@radix-ui/react-icons";
import { useAppDispatch, useAppSelector } from "../../hooks";
import { pop, push } from "../Pages/pagesSlice";
import { BuddyCanvas } from "./BuddyCanvas";
import { BuddyRecentChats } from "./BuddyRecentChats";
import { useBuddyState } from "./hooks/useBuddyState";
import {
  selectBuddySnapshot,
  selectBuddyLoaded,
  selectIsBuddyEnabled,
  selectBuddyActivities,
  selectNowPlaying,
  selectActiveSpeech,
  selectBuddyDiagnostics,
} from "./buddySlice";
import { openChatInModeAndStart } from "../Chat/Thread";
import { executeBuddyAction } from "./executeBuddyAction";
import type { BuddyControl, BuddyCareAction, BuddyNeeds } from "./types";
import { PALETTES, STAGES, SKILLS, SIGNALS } from "./constants";
import { computeXpFill } from "./buddyUtils";
import { useGetStatsSummaryQuery } from "../../services/refact/stats";
import { useGetSetupStatusQuery } from "../../services/refact/setupStatus";
import { SETUP_MODES } from "../Setup/setupModes";
import { useUpdateBuddySettingsMutation } from "../../services/refact/buddy";
import styles from "./BuddyHome.module.css";

const NEED_ROWS: Array<{
  key: keyof BuddyNeeds;
  label: string;
  invert?: boolean;
}> = [
  { key: "hunger", label: "Hunger" },
  { key: "energy", label: "Energy" },
  { key: "hygiene", label: "Hygiene" },
  { key: "boredom", label: "Boredom", invert: true },
  { key: "affection", label: "Affection" },
];

const CARE_ACTIONS: Array<{
  action: BuddyCareAction;
  label: string;
  emoji: string;
  toy?: string;
}> = [
  { action: "feed", label: "Feed", emoji: "🍜" },
  { action: "play", label: "Play", emoji: "🎾", toy: "bug" },
  { action: "pet", label: "Pet", emoji: "💕" },
  { action: "sleep", label: "Sleep", emoji: "😴" },
  { action: "clean", label: "Clean", emoji: "🧼" },
];

export const BuddyHome: React.FC = () => {
  const dispatch = useAppDispatch();
  const snapshot = useAppSelector(selectBuddySnapshot);
  const loaded = useAppSelector(selectBuddyLoaded);
  const enabled = useAppSelector(selectIsBuddyEnabled);
  const activities = useAppSelector(selectBuddyActivities);
  const nowPlaying = useAppSelector(selectNowPlaying);
  const activeSpeech = useAppSelector(selectActiveSpeech);
  const diagnostics = useAppSelector(selectBuddyDiagnostics);
  const buddy = useBuddyState();
  const { state } = buddy;
  const [setupDismissed, setSetupDismissed] = useState(false);
  const [updateSettings, { isLoading: isSavingSettings }] =
    useUpdateBuddySettingsMutation();

  const { data: statsData } = useGetStatsSummaryQuery({});
  const { data: setupData } = useGetSetupStatusQuery(undefined, {
    refetchOnMountOrArgChange: true,
  });
  const setupNeeded = !setupData?.configured && !setupDismissed;

  const paletteIndex =
    snapshot?.state.identity.palette_index ?? state.paletteIndex;
  const palette = PALETTES[paletteIndex] ?? PALETTES[0];

  const progression = snapshot?.state.progression;
  const identity = snapshot?.state.identity;
  const skills = snapshot?.state.skills;
  const semantic = snapshot?.state.semantic;
  const pet = snapshot?.state.pet;
  const personality = snapshot?.state.personality;
  const settings = snapshot?.settings;

  const stage = STAGES[progression?.stage ?? state.progress.stage] ?? STAGES[0];
  const nextStage = STAGES[(progression?.stage ?? state.progress.stage) + 1];

  const xp = progression?.xp ?? state.progress.xp;
  const xpNext = progression?.xp_next ?? nextStage?.xpThreshold;
  const xpFill = useMemo(
    () => computeXpFill(progression?.xp ?? 0, progression?.xp_next ?? 100),
    [progression],
  );

  const name = identity?.name ?? state.name;
  const statusText = semantic?.headline ?? "";
  const needRows = useMemo(
    () =>
      NEED_ROWS.map((item) => {
        const value = pet?.needs[item.key] ?? 0;
        const fill = item.invert ? 100 - value : value;
        return {
          ...item,
          value,
          fill: Math.max(0, Math.min(100, fill)),
        };
      }),
    [pet],
  );

  const handleBack = useCallback(() => {
    dispatch(pop());
  }, [dispatch]);

  const handleSettings = useCallback(() => {
    void updateSettings({ proactive_enabled: !settings?.proactive_enabled });
  }, [settings?.proactive_enabled, updateSettings]);

  const handleViewStats = useCallback(() => {
    dispatch(push({ name: "stats dashboard" }));
  }, [dispatch]);

  const handleRunMode = useCallback(
    (mode: string) => {
      void dispatch(openChatInModeAndStart({ mode }));
    },
    [dispatch],
  );

  const handleDismissSetup = useCallback(() => {
    setSetupDismissed(true);
  }, []);

  const handleCare = useCallback(
    async (action: BuddyCareAction, toy?: string) => {
      await executeBuddyAction(
        {
          id: `care-${action}`,
          label: action,
          action: `care_${action}`,
          action_param: toy,
          style: "primary",
        },
        dispatch,
      );
    },
    [dispatch],
  );

  const handlePromptChange = useCallback(
    async (prompt: string | null) => {
      if (prompt === null) {
        await updateSettings({ clear_personality_prompt: true });
        return;
      }
      await updateSettings({ personality_prompt: prompt });
    },
    [updateSettings],
  );

  const handleReroll = useCallback(async () => {
    await executeBuddyAction(
      {
        id: "reroll-personality",
        label: "Reroll",
        action: "reroll_personality",
        style: "primary",
      },
      dispatch,
    );
  }, [dispatch]);

  const activeDiagnostic = activeSpeech?.chat_id
    ? diagnostics.find((diag) => diag.chat_id === activeSpeech.chat_id)
    : undefined;

  const handleSpeechControl = useCallback(
    async (ctrl: BuddyControl) => {
      if (!activeSpeech) return;
      await executeBuddyAction(ctrl, dispatch, {
        triggerText: activeSpeech.text,
        triggerSource: "runtime",
        sourceChatId: activeSpeech.chat_id,
        diagnostic: activeDiagnostic,
      });
    },
    [dispatch, activeSpeech, activeDiagnostic],
  );

  const unlockedSkills = skills?.unlocked ?? state.skills;

  if (!loaded) {
    return (
      <div className={styles.page}>
        <Flex align="center" justify="center" style={{ flex: 1 }}>
          <Spinner size="3" />
        </Flex>
      </div>
    );
  }

  if (snapshot === null || !enabled) {
    return (
      <div className={styles.page}>
        <div className={styles.topBar}>
          <Button variant="ghost" size="1" onClick={handleBack}>
            <ArrowLeftIcon width={14} height={14} />
            Back
          </Button>
        </div>
        <Flex
          align="center"
          justify="center"
          direction="column"
          gap="2"
          style={{ flex: 1 }}
        >
          <Text size="2" color="gray">
            Buddy is not available
          </Text>
        </Flex>
      </div>
    );
  }

  return (
    <div className={styles.page}>
      <div className={styles.topBar}>
        <Button variant="ghost" size="1" onClick={handleBack}>
          <ArrowLeftIcon width={14} height={14} />
          Back
        </Button>
        <Text size="2" weight="bold" className={styles.topTitle}>
          {stage.emoji} {name}
        </Text>
        <Button variant="ghost" size="1" onClick={handleSettings}>
          <GearIcon width={14} height={14} />
        </Button>
      </div>

      <div className={styles.hero}>
        <div className={styles.scene}>
          <div className={styles.glowWrap}>
            <div
              className={styles.glow}
              style={{ backgroundColor: palette.body }}
            />
            <BuddyCanvas
              state={state}
              onEvent={buddy.handleCanvasEvent}
              displaySize={320}
              speechOverride={
                activeSpeech
                  ? activeSpeech.text
                  : nowPlaying?.speech_text ?? nowPlaying?.title ?? null
              }
              speechControls={activeSpeech ? activeSpeech.controls : undefined}
              onSpeechControlClick={
                activeSpeech ? handleSpeechControl : undefined
              }
            />
          </div>
        </div>

        <div
          className={styles.stageBadge}
          style={{
            backgroundColor: palette.body + "33",
            color: palette.body,
          }}
        >
          {stage.emoji} {stage.name}
        </div>

        {statusText && <div className={styles.statusText}>{statusText}</div>}

        {nowPlaying && nowPlaying.progress != null && (
          <div className={styles.statusBubble}>
            <span className={styles.statusIcon}>
              {SIGNALS[nowPlaying.signal_type]?.icon ?? "⚡"}
            </span>
            <div className={styles.statusContent}>
              <div className={styles.progressBar}>
                <div style={{ width: `${nowPlaying.progress}%` }} />
              </div>
            </div>
          </div>
        )}

        {setupNeeded && (
          <div className={styles.setupChips}>
            {SETUP_MODES.map((m) => (
              <Button
                key={m.mode}
                size="1"
                variant={m.mode === "setup" ? "soft" : "outline"}
                onClick={() => handleRunMode(m.mode)}
              >
                {m.label}
              </Button>
            ))}
            <Button
              size="1"
              variant="ghost"
              color="gray"
              onClick={handleDismissSetup}
            >
              Dismiss
            </Button>
          </div>
        )}

        <div className={styles.careBar}>
          {CARE_ACTIONS.map((item) => (
            <button
              key={item.action}
              type="button"
              className={styles.careButton}
              onClick={() => void handleCare(item.action, item.toy)}
            >
              <span>{item.emoji}</span>
              <span>{item.label}</span>
            </button>
          ))}
        </div>
      </div>

      <div className={styles.needsCard}>
        <Text
          size="1"
          weight="bold"
          color="gray"
          className={styles.sectionLabel}
        >
          CARE LOOP
        </Text>
        <div className={styles.needsGrid}>
          {needRows.map((item) => (
            <div key={item.key} className={styles.needRow}>
              <div className={styles.needHeader}>
                <span>{item.label}</span>
                <span>{item.value}</span>
              </div>
              <div className={styles.needBar}>
                <div
                  className={styles.needFill}
                  style={{ width: `${item.fill}%` }}
                />
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className={styles.personalityCard}>
        <div className={styles.personalityHeader}>
          <div>
            <Text
              size="1"
              weight="bold"
              color="gray"
              className={styles.sectionLabel}
            >
              PERSONALITY
            </Text>
            <Text size="2" weight="bold">
              {personality?.archetype_label ?? "Buddy"}
            </Text>
            <Text size="1" color="gray">
              {personality?.vibe ?? "Playful, quirky, helpful"}
            </Text>
          </div>
          <Button size="1" variant="soft" onClick={() => void handleReroll()}>
            Reroll
          </Button>
        </div>

        <Text size="1" className={styles.personalitySummary}>
          {personality?.summary}
        </Text>

        <div className={styles.traitsGrid}>
          {Object.entries(personality?.traits ?? {}).map(([key, value]) => (
            <div key={key} className={styles.traitRow}>
              <span className={styles.traitName}>{key}</span>
              <span className={styles.traitValue}>{value}</span>
            </div>
          ))}
        </div>

        <div className={styles.settingsRow}>
          <Button
            size="1"
            variant={settings?.proactive_enabled ? "soft" : "outline"}
            onClick={handleSettings}
            disabled={isSavingSettings}
          >
            {settings?.proactive_enabled ? "Proactive On" : "Proactive Off"}
          </Button>
          <Button
            size="1"
            variant="outline"
            onClick={() =>
              void handlePromptChange(
                settings?.personality_prompt
                  ? null
                  : personality?.prompt ?? null,
              )
            }
            disabled={isSavingSettings}
          >
            {settings?.personality_prompt ? "Use Random Vibe" : "Use Current Vibe"}
          </Button>
        </div>
      </div>

      {statsData && (
        <div className={styles.statsSummary}>
          <div className={styles.statItem}>
            <Text size="1" color="gray">
              Messages
            </Text>
            <Text size="2" weight="bold">
              {statsData.totals.total_calls.toLocaleString()}
            </Text>
          </div>
          <div className={styles.statItem}>
            <Text size="1" color="gray">
              Tokens
            </Text>
            <Text size="2" weight="bold">
              {(statsData.totals.total_tokens / 1000).toFixed(1)}k
            </Text>
          </div>
          <div className={styles.statItem}>
            <Text size="1" color="gray">
              Success
            </Text>
            <Text size="2" weight="bold">
              {statsData.totals.total_calls > 0
                ? Math.round(
                    (statsData.totals.successful_calls /
                      statsData.totals.total_calls) *
                      100,
                  )
                : 0}
              %
            </Text>
          </div>
          <Button size="1" variant="ghost" onClick={handleViewStats}>
            View Full Stats →
          </Button>
        </div>
      )}

      <div className={styles.setupActions}>
        <Text
          size="1"
          weight="bold"
          color="gray"
          className={styles.sectionLabel}
        >
          PROJECT SETUP
        </Text>
        <div className={styles.setupActionButtons}>
          {SETUP_MODES.map((m) => (
            <button
              key={m.mode}
              type="button"
              className={styles.setupActionButton}
              onClick={() => handleRunMode(m.mode)}
            >
              <Text size="1">{m.label}</Text>
            </button>
          ))}
        </div>
      </div>

      <div className={styles.infoGrid}>
        <div className={styles.infoPanel}>
          <Text
            size="1"
            weight="bold"
            color="gray"
            className={styles.sectionLabel}
          >
            STATUS
          </Text>
          <Flex direction="column" gap="1">
            <Flex justify="between">
              <Text size="1" color="gray">
                Stage
              </Text>
              <Text size="1" weight="bold">
                {stage.name}
              </Text>
            </Flex>
            <Flex justify="between">
              <Text size="1" color="gray">
                Growth
              </Text>
              <Text size="1" weight="bold">
                {xp} {xpNext ? `/ ${xpNext}` : "(max)"}
              </Text>
            </Flex>
            {pet && (
              <Flex justify="between">
                <Text size="1" color="gray">
                  Care score
                </Text>
                <Text size="1" weight="bold">
                  {pet.evolution.care_score}
                </Text>
              </Flex>
            )}
            {pet && (
              <Flex justify="between">
                <Text size="1" color="gray">
                  Neglect
                </Text>
                <Text size="1" weight="bold">
                  {pet.evolution.neglect_score}
                </Text>
              </Flex>
            )}
          </Flex>
          <div className={styles.xpBar}>
            <div className={styles.xpFill} style={{ width: `${xpFill}%` }} />
          </div>
          <Text
            size="1"
            weight="bold"
            color="gray"
            className={styles.sectionLabel}
            style={{ marginTop: "var(--space-1)" }}
          >
            SKILLS
          </Text>
          <Flex wrap="wrap" gap="1">
            {unlockedSkills.length === 0 && (
              <Text size="1" color="gray">
                None yet
              </Text>
            )}
            {unlockedSkills.map((id) => {
              const skill = SKILLS.find((s) => s.id === id);
              return skill ? (
                <span key={id} className={styles.skillChip}>
                  {skill.icon} {skill.name}
                </span>
              ) : null;
            })}
          </Flex>
          {semantic?.last_active && (
            <Flex justify="between">
              <Text size="1" color="gray">
                Last active
              </Text>
              <Text size="1">
                {new Date(semantic.last_active).toLocaleDateString()}
              </Text>
            </Flex>
          )}
        </div>

        <div className={styles.infoPanel}>
          <Text
            size="1"
            weight="bold"
            color="gray"
            className={styles.sectionLabel}
          >
            ACTIVITY
          </Text>
          {activities.length === 0 && (
            <Text size="1" color="gray">
              No recent activity
            </Text>
          )}
          {activities.slice(0, 6).map((a, i) => (
            <div key={i} className={styles.activityItem}>
              <span className={styles.activityIcon}>{a.icon}</span>
              <span className={styles.activityDesc}>{a.title}</span>
              <span className={styles.activityTime}>
                {a.timestamp
                  ? new Date(a.timestamp).toLocaleTimeString([], {
                      hour: "2-digit",
                      minute: "2-digit",
                    })
                  : ""}
              </span>
            </div>
          ))}
        </div>
      </div>

      {snapshot?.state.workflow_summaries &&
        snapshot.state.workflow_summaries.length > 0 && (
          <div className={styles.workflowsSection}>
            <Text
              size="1"
              weight="bold"
              color="gray"
              className={styles.sectionLabel}
            >
              RECENT WORKFLOWS
            </Text>
            {snapshot.state.workflow_summaries.map((w) => (
              <div key={w.workflow_id} className={styles.workflowItem}>
                <span className={styles.workflowIcon}>
                  {w.last_outcome === "success"
                    ? "✅"
                    : w.last_outcome === "failed"
                      ? "❌"
                      : "⚙️"}
                </span>
                <span className={styles.workflowName}>
                  {w.workflow_id.replace(/_/g, " ")}
                </span>
                <span className={styles.workflowMeta}>
                  ×{w.run_count}
                  {w.last_run
                    ? ` · ${new Date(w.last_run).toLocaleDateString()}`
                    : ""}
                </span>
              </div>
            ))}
          </div>
        )}

      <div className={styles.chatsSection}>
        <BuddyRecentChats />
      </div>
    </div>
  );
};
