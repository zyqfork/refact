import React, { useRef, useEffect, useCallback, useState } from "react";
import { createInitialAnimState } from "./state";
import { renderFrame } from "./canvas/render";
import {
  stepAnimFrame,
  triggerSignalAnimation,
  handlePet,
} from "./canvas/animLoop";
import {
  CANVAS_SIZE,
  CANVAS_CENTER_X,
  CANVAS_CENTER_Y,
  STAGE_SIZES,
  PALETTES,
} from "./constants";
import type {
  BuddyCanvasProps,
  BuddyAnimState,
  BuddySemanticState,
  BuddyEvent,
  BubblePosition,
} from "./types";

const BUBBLE_STYLES: Record<
  BubblePosition,
  {
    container: React.CSSProperties;
    tail: React.CSSProperties;
  }
> = {
  top: {
    container: {
      bottom: "60%",
      left: "calc(50% + var(--buddy-walk-x, 0px))",
      transform: "translateX(-50%)",
    },
    tail: {
      top: "100%",
      left: "50%",
      transform: "translateX(-50%)",
      borderLeft: "11px solid transparent",
      borderRight: "11px solid transparent",
      /* borderTop set dynamically via palette */
    },
  },
  left: {
    container: {
      right: "calc(56% - var(--buddy-walk-x, 0px))",
      top: "42%",
      marginRight: "-6px",
      transform: "translateY(-50%)",
    },
    tail: {
      left: "100%",
      top: "50%",
      transform: "translateY(-50%)",
      borderTop: "11px solid transparent",
      borderBottom: "11px solid transparent",
      /* borderLeft set dynamically via palette */
    },
  },
  right: {
    container: {
      left: "calc(56% + var(--buddy-walk-x, 0px))",
      top: "42%",
      marginLeft: "-6px",
      transform: "translateY(-50%)",
    },
    tail: {
      right: "100%",
      top: "50%",
      transform: "translateY(-50%)",
      borderTop: "11px solid transparent",
      borderBottom: "11px solid transparent",
      /* borderRight set dynamically via palette */
    },
  },
};

const BUBBLE_POSITIONS: BubblePosition[] = ["top", "left", "right"];

function randomBubblePosition(previous?: BubblePosition): BubblePosition {
  const choices = previous
    ? BUBBLE_POSITIONS.filter((position) => position !== previous)
    : BUBBLE_POSITIONS;
  return choices[Math.floor(Math.random() * choices.length)] ?? "top";
}

export const BuddyCanvas: React.FC<BuddyCanvasProps> = ({
  state,
  onEvent,
  displaySize = 512,
  className,
  style,
  speechOverride,
  speechControls,
  onSpeechControlClick,
  bubblePosition = "top",
  randomizeBubblePosition = false,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animRef = useRef<BuddyAnimState>(createInitialAnimState());
  const semanticRef = useRef<BuddySemanticState>(state);
  const prevSignalTimeRef = useRef<number>(0);
  const frameIdRef = useRef<number>(0);
  const bubbleRef = useRef<HTMLDivElement>(null);
  const bubbleTextRef = useRef<string>("");
  const [activeBubblePosition, setActiveBubblePosition] =
    useState<BubblePosition>(bubblePosition);
  const activeBubblePositionRef = useRef<BubblePosition>(bubblePosition);
  const bubblePositionRef = useRef<BubblePosition>(bubblePosition);
  const speechOverrideRef = useRef<string | null | undefined>(speechOverride);

  useEffect(() => {
    speechOverrideRef.current = speechOverride;
  }, [speechOverride]);

  useEffect(() => {
    bubblePositionRef.current = bubblePosition;
    if (!randomizeBubblePosition) {
      activeBubblePositionRef.current = bubblePosition;
      setActiveBubblePosition(bubblePosition);
    }
  }, [bubblePosition, randomizeBubblePosition]);

  const palette = PALETTES[state.paletteIndex] ?? PALETTES[0];

  useEffect(() => {
    semanticRef.current = state;
  }, [state]);

  const emit = useCallback(
    (event: BuddyEvent) => {
      onEvent?.(event);
    },
    [onEvent],
  );

  useEffect(() => {
    const { lastSignalTime, lastSignalType } = state.activity;
    if (
      lastSignalTime !== prevSignalTimeRef.current &&
      lastSignalTime > 0 &&
      lastSignalType
    ) {
      prevSignalTimeRef.current = lastSignalTime;
      triggerSignalAnimation(animRef.current, lastSignalType, emit);
    }
  }, [state.activity, emit]);

  useEffect(() => {
    const loop = () => {
      const ctx = canvasRef.current?.getContext("2d");
      if (ctx) {
        const sem = semanticRef.current;
        stepAnimFrame(animRef.current, sem, emit);
        renderFrame(ctx, animRef.current, sem);
        if (bubbleRef.current) {
          const anim = animRef.current;
          const walkOffsetPx = Math.round(
            (anim.walkOffsetX / CANVAS_SIZE) * displaySize,
          );
          bubbleRef.current.style.setProperty(
            "--buddy-walk-x",
            `${walkOffsetPx}px`,
          );
          const overrideText = speechOverrideRef.current ?? "";
          const rawText = overrideText || anim.statusText || "";
          const text =
            rawText.length > 140
              ? `${rawText.slice(0, 137).trimEnd()}…`
              : rawText;
          const opacity = overrideText ? 1 : anim.statusOpacity;
          if (text !== bubbleTextRef.current) {
            bubbleTextRef.current = text;
            const hasControls = Boolean(speechControls?.length);
            const isLongText = text.length > 72;
            const isMediumText = text.length > 34;
            const fixedWidth = hasControls || isLongText;
            bubbleRef.current.style.width = fixedWidth
              ? "270px"
              : isMediumText
                ? "220px"
                : "max-content";
            bubbleRef.current.style.whiteSpace = fixedWidth
              ? "normal"
              : isMediumText
                ? "normal"
                : "nowrap";
            const nextPosition = fixedWidth
              ? "top"
              : randomizeBubblePosition
                ? randomBubblePosition(activeBubblePositionRef.current)
                : bubblePositionRef.current;
            if (nextPosition !== activeBubblePositionRef.current) {
              activeBubblePositionRef.current = nextPosition;
              setActiveBubblePosition(nextPosition);
            }
            const span = bubbleRef.current
              .firstElementChild as HTMLElement | null;
            if (span) span.textContent = text;
          }
          if (opacity > 0.02 && text) {
            bubbleRef.current.style.opacity = String(Math.min(1, opacity));
            bubbleRef.current.style.visibility = "visible";
          } else {
            bubbleRef.current.style.opacity = "0";
            bubbleRef.current.style.visibility = "hidden";
          }
        }
      }
      frameIdRef.current = requestAnimationFrame(loop);
    };
    frameIdRef.current = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frameIdRef.current);
  }, [displaySize, emit, randomizeBubblePosition, speechControls?.length]);

  const toCanvasCoords = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return null;
      return {
        x: ((e.clientX - rect.left) / rect.width) * CANVAS_SIZE,
        y: ((e.clientY - rect.top) / rect.height) * CANVAS_SIZE,
        normX: ((e.clientX - rect.left) / rect.width) * 2 - 1,
        normY: ((e.clientY - rect.top) / rect.height) * 2 - 1,
      };
    },
    [],
  );

  const onMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = toCanvasCoords(e);
      if (!coords) return;
      const anim = animRef.current;
      anim.mouseSpeed = Math.sqrt(
        (coords.normX - anim.cursorTargetX) ** 2 +
          (coords.normY - anim.cursorTargetY) ** 2,
      );
      anim.cursorTargetX = coords.normX;
      anim.cursorTargetY = coords.normY;
      const stage = semanticRef.current.progress.stage;
      const [spriteW] = STAGE_SIZES[stage] ?? [28, 18];
      const buddyX = CANVAS_CENTER_X + anim.walkOffsetX;
      const dist = Math.sqrt(
        (coords.x - buddyX) ** 2 + (coords.y - CANVAS_CENTER_Y) ** 2,
      );
      anim.mouseOnBuddy = dist < spriteW / 2 + 4;
      const dx = (coords.normX * CANVAS_SIZE) / 2;
      const dy = (coords.normY * CANVAS_SIZE) / 2;
      anim.mouseProximity = Math.max(0, 1 - Math.sqrt(dx * dx + dy * dy) / 80);
      anim.mouseAngle = Math.atan2(coords.normY, coords.normX);
    },
    [toCanvasCoords],
  );

  const onMouseLeave = useCallback(() => {
    const anim = animRef.current;
    anim.mouseOnBuddy = false;
    anim.mouseProximity = 0;
    anim.mouseNearTimer = 0;
    anim.dragging = false;
  }, []);

  const onMouseDown = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = toCanvasCoords(e);
      if (!coords) return;
      const stage = semanticRef.current.progress.stage;
      const [spriteW] = STAGE_SIZES[stage] ?? [28, 18];
      const hitRadius = spriteW / 2 + 4;
      const buddyX = CANVAS_CENTER_X + animRef.current.walkOffsetX;
      if (
        Math.sqrt(
          (coords.x - buddyX) ** 2 + (coords.y - CANVAS_CENTER_Y) ** 2,
        ) < hitRadius
      ) {
        animRef.current.dragging = true;
      }
    },
    [toCanvasCoords],
  );

  const onMouseUp = useCallback(() => {
    const anim = animRef.current;
    if (anim.dragging) {
      anim.dragging = false;
      anim.squashTargetX = 1.1;
      anim.squashTargetY = 0.9;
    }
  }, []);

  const onClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const coords = toCanvasCoords(e);
      if (!coords) return;
      const stage = semanticRef.current.progress.stage;
      handlePet(animRef.current, coords.x, coords.y, emit, stage);
    },
    [toCanvasCoords, emit],
  );

  return (
    <div
      className={className}
      style={{
        position: "relative",
        display: "inline-block",
        width: displaySize,
        height: displaySize,
        flexShrink: 0,
        ...style,
      }}
    >
      <canvas
        ref={canvasRef}
        width={CANVAS_SIZE}
        height={CANVAS_SIZE}
        style={{
          width: displaySize,
          height: displaySize,
          imageRendering: "pixelated",
          display: "block",
          cursor: "pointer",
        }}
        onMouseMove={onMouseMove}
        onMouseLeave={onMouseLeave}
        onMouseDown={onMouseDown}
        onMouseUp={onMouseUp}
        onClick={onClick}
      />
      {displaySize >= 100 &&
        (() => {
          const pos = BUBBLE_STYLES[activeBubblePosition];
          const tailColor: React.CSSProperties =
            activeBubblePosition === "left"
              ? { borderLeft: `13px solid ${palette.body}` }
              : activeBubblePosition === "right"
                ? { borderRight: `13px solid ${palette.body}` }
                : { borderTop: `13px solid ${palette.body}` };
          return (
            <div
              ref={bubbleRef}
              data-bubble-position={activeBubblePosition}
              style={{
                position: "absolute",
                ...pos.container,
                background: "rgba(12, 20, 34, 0.88)",
                border: `2px solid ${palette.body}`,
                borderRadius: "14px",
                padding: "7px 12px",
                fontSize: "11px",
                fontFamily:
                  "system-ui, -apple-system, BlinkMacSystemFont, sans-serif",
                fontWeight: 700,
                letterSpacing: "0.1px",
                lineHeight: 1.3,
                whiteSpace: speechControls?.length ? "normal" : "nowrap",
                width: speechControls?.length ? "270px" : "max-content",
                maxWidth: "270px",
                overflowWrap: "break-word",
                pointerEvents: speechControls?.length ? "auto" : "none",
                color: palette.light,
                boxShadow: `0 8px 22px rgba(0, 0, 0, 0.26), 0 0 18px ${palette.dark}44`,
                zIndex: 5,
                visibility: "hidden",
                opacity: 0,
              }}
            >
              <span />
              {speechControls?.length ? (
                <div
                  style={{
                    display: "flex",
                    gap: "5px",
                    flexWrap: "wrap",
                    marginTop: "7px",
                  }}
                >
                  {speechControls.map((ctrl) => (
                    <button
                      key={ctrl.id}
                      onClick={(e) => {
                        e.stopPropagation();
                        onSpeechControlClick?.(ctrl);
                      }}
                      style={{
                        background:
                          ctrl.style === "primary"
                            ? palette.body
                            : "rgba(255, 255, 255, 0.08)",
                        border: `1px solid ${palette.body}`,
                        borderRadius: "999px",
                        color:
                          ctrl.style === "primary" ? "#0d0d16" : palette.light,
                        fontFamily:
                          "system-ui, -apple-system, BlinkMacSystemFont, sans-serif",
                        fontWeight: 700,
                        fontSize: "10px",
                        padding: "3px 8px",
                        cursor: "pointer",
                        letterSpacing: "0.1px",
                      }}
                    >
                      {ctrl.label}
                    </button>
                  ))}
                </div>
              ) : null}
              <div
                style={{
                  position: "absolute",
                  width: 0,
                  height: 0,
                  ...pos.tail,
                  ...tailColor,
                  filter: `drop-shadow(0 0 3px ${palette.dark})`,
                }}
              />
            </div>
          );
        })()}
    </div>
  );
};
