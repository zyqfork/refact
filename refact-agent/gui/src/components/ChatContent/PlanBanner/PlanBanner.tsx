import React, { useEffect, useMemo, useState } from "react";
import { Box, Button, Flex, Text } from "@radix-ui/themes";
import { useAppSelector } from "../../../hooks";
import { selectPlanBannerState } from "../../../features/Chat/Thread/selectors";
import { Markdown } from "../../Markdown";
import styles from "./PlanBanner.module.css";
import { getPlanMetadata } from "../../../services/refact/types";
import { PlanHistoryModal } from "./PlanHistoryModal";

type PlanBannerProps = {
  threadId: string;
};

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

function humanizedAgeFrom(
  createdAtMs: number | undefined,
  nowMs: number,
): string {
  if (createdAtMs === undefined) return "recently";
  const ageMs = Math.max(0, nowMs - createdAtMs);
  if (!Number.isFinite(ageMs)) return "recently";
  if (ageMs < MINUTE_MS) return "just now";
  if (ageMs < HOUR_MS) return `${Math.floor(ageMs / MINUTE_MS)}m ago`;
  if (ageMs < DAY_MS) return `${Math.floor(ageMs / HOUR_MS)}h ago`;
  if (ageMs < 2 * DAY_MS) return "yesterday";
  return `${Math.floor(ageMs / DAY_MS)} days ago`;
}

function storageKey(threadId: string): string {
  return `plan-banner-collapsed-${threadId}`;
}

function readCollapsed(threadId: string): boolean {
  try {
    if (typeof localStorage === "undefined") return false;
    return localStorage.getItem(storageKey(threadId)) === "true";
  } catch {
    return false;
  }
}

function writeCollapsed(threadId: string, collapsed: boolean): void {
  try {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(storageKey(threadId), String(collapsed));
  } catch {
    return;
  }
}

export const PlanBanner: React.FC<PlanBannerProps> = ({ threadId }) => {
  const {
    base: plan,
    synthesizedText,
    history: planHistory,
  } = useAppSelector((state) => selectPlanBannerState(state, threadId));
  const [collapsed, setCollapsed] = useState(() => readCollapsed(threadId));
  const [historyOpen, setHistoryOpen] = useState(false);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const metadata = useMemo(
    () => (plan ? getPlanMetadata(plan) : undefined),
    [plan],
  );

  useEffect(() => {
    setCollapsed(readCollapsed(threadId));
  }, [threadId]);

  useEffect(() => {
    setNowMs(Date.now());
  }, [metadata?.created_at_ms]);

  useEffect(() => {
    setHistoryOpen(false);
  }, [threadId]);

  const title = useMemo(() => {
    if (!plan) return "";
    const mode = metadata?.mode ?? "Mode unknown";
    const version =
      metadata?.version !== undefined ? `v${metadata.version}` : "v?";
    return `📋 Plan — ${mode} · ${version} · ${humanizedAgeFrom(
      metadata?.created_at_ms,
      nowMs,
    )}`;
  }, [metadata, nowMs, plan]);

  if (!plan) return null;

  const handleToggle = () => {
    const nextCollapsed = !collapsed;
    setCollapsed(nextCollapsed);
    writeCollapsed(threadId, nextCollapsed);
  };

  const handleHistoryClick = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.stopPropagation();
    setHistoryOpen(true);
  };

  return (
    <Box className={styles.sticky} data-testid="plan-banner">
      <Box className={styles.card}>
        <Flex
          align="center"
          gap="2"
          className={styles.header}
          onClick={handleToggle}
          data-testid="plan-banner-header"
        >
          <span className={styles.icon}>📋</span>
          <Text size="1" className={styles.title}>
            {title}
          </Text>
          <Button
            type="button"
            size="1"
            variant="ghost"
            color="gray"
            onClick={handleHistoryClick}
          >
            History
          </Button>
        </Flex>
        {!collapsed && (
          <Box className={styles.body} data-testid="plan-banner-body">
            <Markdown>{synthesizedText ?? plan.content}</Markdown>
          </Box>
        )}
      </Box>
      <PlanHistoryModal
        open={historyOpen}
        onOpenChange={setHistoryOpen}
        items={planHistory}
      />
    </Box>
  );
};
