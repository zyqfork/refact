import React from "react";
import { BuddyCanvas } from "./BuddyCanvas";
import type {
  BuddyControl,
  BuddyEvent,
  BuddySemanticState,
  BubblePosition,
  Palette,
  Stage,
} from "./types";
import styles from "./BuddyWorld.module.css";

interface BuddyCharacterProps {
  state: BuddySemanticState;
  stage: Stage;
  palette: Palette;
  displaySize: number;
  showStageBadge?: boolean;
  bubblePosition?: BubblePosition;
  randomizeBubblePosition?: boolean;
  sceneXPercent?: number;
  roamTargetX?: number;
  roamBoost?: number;
  speechText?: string | null;
  speechControls?: BuddyControl[];
  onCanvasEvent: (event: BuddyEvent) => void;
  onSpeechControl?: (control: BuddyControl) => void;
}

export const BuddyCharacter: React.FC<BuddyCharacterProps> = ({
  state,
  stage,
  palette,
  displaySize,
  showStageBadge = false,
  bubblePosition = "top",
  randomizeBubblePosition = false,
  sceneXPercent,
  roamTargetX,
  roamBoost,
  speechText,
  speechControls,
  onCanvasEvent,
  onSpeechControl,
}) => (
  <div
    className={styles.character}
    style={
      typeof sceneXPercent === "number"
        ? { left: `${sceneXPercent}%` }
        : undefined
    }
    data-testid="buddy-world-character"
  >
    <BuddyCanvas
      state={state}
      onEvent={onCanvasEvent}
      displaySize={displaySize}
      speechOverride={speechText}
      speechControls={speechControls}
      onSpeechControlClick={onSpeechControl}
      bubblePosition={bubblePosition}
      randomizeBubblePosition={randomizeBubblePosition}
      roamTargetX={roamTargetX}
      roamBoost={roamBoost}
    />
    {showStageBadge && (
      <div
        className={styles.stageBadge}
        style={{ borderColor: palette.body, color: palette.body }}
      >
        {stage.emoji} {stage.name}
      </div>
    )}
  </div>
);
