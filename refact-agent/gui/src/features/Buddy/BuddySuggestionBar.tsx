import React, { useCallback } from "react";
import { Flex, Text, Button } from "@radix-ui/themes";
import { useAppDispatch, useAppSelector } from "../../hooks";
import { useDismissBuddySuggestionMutation } from "../../services/refact/buddy";
import {
  selectBuddySuggestions,
  dismissBuddySuggestion,
} from "./buddySlice";
import { useBuddyState } from "./hooks/useBuddyState";
import { BuddyCanvas } from "./BuddyCanvas";
import { PALETTES } from "./constants";
import type { BuddySuggestion } from "./types";
import styles from "./BuddySuggestionBar.module.css";

interface SuggestionCardProps {
  suggestion: BuddySuggestion;
}

const SuggestionCard: React.FC<SuggestionCardProps> = ({ suggestion }) => {
  const dispatch = useAppDispatch();
  const [dismissMutation] = useDismissBuddySuggestionMutation();
  const buddy = useBuddyState();
  const palette = PALETTES[buddy.state.paletteIndex] ?? PALETTES[0];

  const handleDismiss = useCallback(async () => {
    await dismissMutation(suggestion.id);
    dispatch(dismissBuddySuggestion(suggestion.id));
  }, [dismissMutation, dispatch, suggestion.id]);

  return (
    <div className={styles.card} style={{ borderColor: palette.body }}>
      <div className={styles.canvasWrap}>
        <BuddyCanvas
          state={buddy.state}
          onEvent={buddy.handleCanvasEvent}
          style={{ width: 48, height: 48 }}
        />
      </div>
      <div className={styles.bubble} style={{ borderColor: palette.body }}>
        <Text size="1" weight="bold" className={styles.title}>
          {suggestion.title}
        </Text>
        <Text size="1" color="gray" className={styles.desc}>
          {suggestion.description}
        </Text>
      </div>
      <Flex gap="1" align="center" className={styles.actions}>
        <Button size="1" variant="soft" color="gray" onClick={handleDismiss}>
          Ignore
        </Button>
      </Flex>
    </div>
  );
};

export const BuddySuggestionBar: React.FC = () => {
  const suggestions = useAppSelector(selectBuddySuggestions);
  const active = suggestions.filter((s) => !s.dismissed).slice(0, 1);

  if (active.length === 0) return null;

  return (
    <div className={styles.bar}>
      {active.map((s) => (
        <SuggestionCard key={s.id} suggestion={s} />
      ))}
    </div>
  );
};
