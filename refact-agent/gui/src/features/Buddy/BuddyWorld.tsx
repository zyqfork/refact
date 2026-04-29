import React, { useEffect, useMemo, useRef, useState } from "react";
import { Text } from "@radix-ui/themes";
import classNames from "classnames";
import { BuddyCharacter } from "./BuddyCharacter";
import type {
  BuddyCareAction,
  BuddyControl,
  BuddyEvent,
  BuddyPage,
  BuddyPetState,
  BuddyPulse,
  BuddyQuest,
  BuddyRuntimeEvent,
  BuddySemanticState,
  Palette,
  Stage,
} from "./types";
import {
  buildBuddyWorldState,
  type BuddyWorldObject,
  type BuddyWorldState,
  type BuddyWorldTone,
} from "./buddyWorldModel";
import styles from "./BuddyWorld.module.css";

interface BuddyWorldProps {
  palette: Palette;
  stage: Stage;
  state: BuddySemanticState;
  pulse: BuddyPulse | null | undefined;
  pet: BuddyPetState | undefined;
  nowPlaying: BuddyRuntimeEvent | null;
  activeQuest: BuddyQuest | null;
  activeSpeech: {
    text: string;
    controls: BuddyControl[];
    chat_id?: string;
  } | null;
  setupNeeded: boolean;
  compact?: boolean;
  onCanvasEvent: (event: BuddyEvent) => void;
  onCare: (action: BuddyCareAction, toy?: string) => void;
  onOpenPage: (page: BuddyPage) => void;
  onRunMode: (mode: string) => void;
  onDismissSetup: () => void;
  onSpeechControl: (control: BuddyControl) => void;
  now?: Date;
}

const TONE_CLASS: Record<BuddyWorldTone, string> = {
  good: styles.toneGood,
  neutral: styles.toneNeutral,
  warning: styles.toneWarning,
  danger: styles.toneDanger,
};

const VITALITY_CLASS: Record<BuddyWorldState["vitality"], string> = {
  lush: styles.vitalityLush,
  growing: styles.vitalityGrowing,
  tangled: styles.vitalityTangled,
};

const SETUP_MODE_ACTIONS = [
  { mode: "setup", label: "Warm up" },
  { mode: "setup_mcp", label: "Link MCP" },
  { mode: "setup_skills", label: "Teach skills" },
] as const;

function pctX(width: number, value: number): number {
  return (width * value) / 100;
}

function pctY(height: number, value: number): number {
  return (height * value) / 100;
}

function toneColor(tone: BuddyWorldTone): string {
  switch (tone) {
    case "good":
      return "#22C55E";
    case "warning":
      return "#F59E0B";
    case "danger":
      return "#EF4444";
    case "neutral":
      return "#60A5FA";
  }
}

function fillPixelRect(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  color: string,
): void {
  ctx.fillStyle = color;
  ctx.fillRect(
    Math.round(x),
    Math.round(y),
    Math.round(width),
    Math.round(height),
  );
}

function drawPixelText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  color: string,
  align: CanvasTextAlign = "center",
): void {
  ctx.save();
  ctx.font = "10px monospace";
  ctx.textAlign = align;
  ctx.textBaseline = "middle";
  ctx.fillStyle = color;
  ctx.fillText(text, x, y);
  ctx.restore();
}

function drawCloud(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  scale: number,
  color: string,
): void {
  fillPixelRect(ctx, x, y + 8 * scale, 34 * scale, 10 * scale, color);
  fillPixelRect(
    ctx,
    x + 6 * scale,
    y + 2 * scale,
    10 * scale,
    8 * scale,
    color,
  );
  fillPixelRect(ctx, x + 16 * scale, y, 12 * scale, 10 * scale, color);
  fillPixelRect(
    ctx,
    x + 28 * scale,
    y + 5 * scale,
    9 * scale,
    9 * scale,
    color,
  );
}

function drawCelestial(
  ctx: CanvasRenderingContext2D,
  world: BuddyWorldState,
  frame: number,
  width: number,
  height: number,
): void {
  const x = pctX(width, world.celestialX);
  const y = pctY(height, world.celestialY) + Math.sin(frame / 34) * 2;
  const isNight = world.phase === "night";
  const color = isNight ? "#E0E7FF" : "#FBBF24";
  const glow = isNight ? "rgba(129,140,248,0.24)" : "rgba(251,191,36,0.26)";

  ctx.fillStyle = glow;
  ctx.beginPath();
  ctx.arc(x, y, isNight ? 34 : 42, 0, Math.PI * 2);
  ctx.fill();

  fillPixelRect(ctx, x - 13, y - 13, 26, 26, color);
  fillPixelRect(ctx, x - 18, y - 8, 36, 16, color);
  fillPixelRect(ctx, x - 8, y - 18, 16, 36, color);

  if (isNight) {
    fillPixelRect(ctx, x + 4, y - 13, 14, 26, "#4C1D95");
  } else {
    fillPixelRect(ctx, x - 2, y - 32, 4, 8, "#F59E0B");
    fillPixelRect(ctx, x - 2, y + 24, 4, 8, "#F59E0B");
    fillPixelRect(ctx, x - 32, y - 2, 8, 4, "#F59E0B");
    fillPixelRect(ctx, x + 24, y - 2, 8, 4, "#F59E0B");
  }
}

function drawWeather(
  ctx: CanvasRenderingContext2D,
  world: BuddyWorldState,
  frame: number,
  width: number,
  height: number,
): void {
  const x = pctX(width, world.weatherX);
  const y = pctY(height, world.weatherY);

  if (world.weather === "storm") {
    drawCloud(ctx, x - 40, y - 15, 1.5, "#475569");
    fillPixelRect(ctx, x + 4, y + 26, 8, 22, "#FACC15");
    fillPixelRect(ctx, x - 2, y + 40, 8, 16, "#FACC15");
    for (let i = 0; i < 16; i += 1) {
      const rx = x - 60 + ((i * 17 + frame * 2) % 130);
      const ry = y + 18 + ((i * 11 + frame) % 64);
      fillPixelRect(ctx, rx, ry, 2, 8, "rgba(125,211,252,0.75)");
    }
    return;
  }

  if (world.weather === "rain") {
    drawCloud(ctx, x - 45, y - 10, 1.45, "#94A3B8");
    for (let i = 0; i < 18; i += 1) {
      const rx = x - 54 + ((i * 13 + frame) % 112);
      const ry = y + 18 + ((i * 19 + frame * 2) % 72);
      fillPixelRect(ctx, rx, ry, 2, 7, "rgba(56,189,248,0.72)");
    }
    return;
  }

  if (world.weather === "wind") {
    for (let i = 0; i < 5; i += 1) {
      const wx = x - 70 + ((frame * (1 + i * 0.22) + i * 36) % 150);
      const wy = y + i * 12;
      fillPixelRect(ctx, wx, wy, 36, 2, "rgba(255,255,255,0.52)");
      fillPixelRect(ctx, wx + 28, wy + 3, 18, 2, "rgba(255,255,255,0.38)");
    }
    return;
  }

  if (world.weather === "busy") {
    ctx.strokeStyle = "rgba(96,165,250,0.72)";
    ctx.lineWidth = 3;
    for (let i = 0; i < 3; i += 1) {
      ctx.beginPath();
      ctx.arc(
        x,
        y,
        16 + i * 13 + Math.sin(frame / 20 + i) * 3,
        0,
        Math.PI * 1.5,
      );
      ctx.stroke();
    }
    return;
  }

  if (world.weather === "dream") {
    for (let i = 0; i < 4; i += 1) {
      drawPixelText(
        ctx,
        "Z",
        x + i * 20,
        y + Math.sin(frame / 16 + i) * 8,
        "#C4B5FD",
      );
    }
    return;
  }

  if (world.weather === "aurora") {
    for (let i = 0; i < 4; i += 1) {
      ctx.strokeStyle =
        i % 2 === 0 ? "rgba(45,212,191,0.45)" : "rgba(168,85,247,0.45)";
      ctx.lineWidth = 8;
      ctx.beginPath();
      ctx.moveTo(0, y + i * 10);
      ctx.bezierCurveTo(
        width * 0.28,
        y - 28 + i * 8,
        width * 0.6,
        y + 36 - i * 6,
        width,
        y - 8 + i * 8,
      );
      ctx.stroke();
    }
  }
}

function drawObject(
  ctx: CanvasRenderingContext2D,
  item: BuddyWorldObject,
  frame: number,
  width: number,
  height: number,
): void {
  const x = pctX(width, item.x);
  const y = pctY(height, item.y);
  const tone = toneColor(item.tone);
  const pulse = Math.sin(frame / 24 + item.x) * 2;

  ctx.fillStyle = `${tone}33`;
  ctx.beginPath();
  ctx.arc(x, y + 14, item.size + 18, 0, Math.PI * 2);
  ctx.fill();

  switch (item.sprite) {
    case "task_grove":
      fillPixelRect(ctx, x - 5, y - 3, 10, 32, "#7C2D12");
      fillPixelRect(ctx, x - 20, y - 20 + pulse, 40, 24, tone);
      fillPixelRect(ctx, x - 13, y - 30 + pulse, 26, 16, "#86EFAC");
      break;
    case "memory_fireflies":
      for (let i = 0; i < 6; i += 1) {
        const fx = x + Math.sin(frame / 18 + i) * (8 + i * 2);
        const fy = y + Math.cos(frame / 15 + i) * 12;
        fillPixelRect(ctx, fx, fy, 4, 4, i % 2 === 0 ? "#FDE68A" : tone);
      }
      fillPixelRect(ctx, x - 12, y + 16, 24, 10, "#854D0E");
      break;
    case "observatory":
      fillPixelRect(ctx, x - 24, y + 12, 48, 20, "#334155");
      fillPixelRect(ctx, x - 18, y, 36, 18, "#64748B");
      fillPixelRect(ctx, x - 4, y - 17, 8, 17, tone);
      fillPixelRect(ctx, x + 4, y - 12, 26, 7, "#CBD5E1");
      break;
    case "satellite":
      fillPixelRect(ctx, x - 8, y - 5 + pulse, 16, 10, "#CBD5E1");
      fillPixelRect(ctx, x - 26, y - 3 + pulse, 14, 6, tone);
      fillPixelRect(ctx, x + 12, y - 3 + pulse, 14, 6, tone);
      fillPixelRect(ctx, x - 1, y + 5 + pulse, 2, 18, "#94A3B8");
      break;
    case "git_vane":
      fillPixelRect(ctx, x - 2, y - 18, 4, 42, "#94A3B8");
      fillPixelRect(ctx, x - 14, y - 14, 12, 4, tone);
      fillPixelRect(ctx, x + 2, y - 5, 16, 4, tone);
      fillPixelRect(ctx, x - 8, y + 10, 20, 4, tone);
      break;
    case "market_comet":
      fillPixelRect(ctx, x - 10, y - 8 + pulse, 20, 16, "#A855F7");
      fillPixelRect(ctx, x - 6, y - 4 + pulse, 12, 8, "#FDE68A");
      fillPixelRect(ctx, x - 34, y + pulse, 20, 4, "rgba(253,186,116,0.7)");
      break;
    case "seed":
      fillPixelRect(ctx, x - 3, y, 6, 20, "#15803D");
      fillPixelRect(ctx, x - 15, y - 12, 14, 10, "#22C55E");
      fillPixelRect(ctx, x + 1, y - 16, 15, 10, "#86EFAC");
      break;
  }
}

function drawScene(
  ctx: CanvasRenderingContext2D,
  world: BuddyWorldState,
  frame: number,
  width: number,
  height: number,
): void {
  ctx.clearRect(0, 0, width, height);
  ctx.imageSmoothingEnabled = false;

  const gradient = ctx.createLinearGradient(0, 0, 0, height);
  if (world.phase === "night") {
    gradient.addColorStop(0, "#111827");
    gradient.addColorStop(0.55, "#312E81");
    gradient.addColorStop(1, "#064E3B");
  } else if (world.phase === "evening") {
    gradient.addColorStop(0, "#7C2D12");
    gradient.addColorStop(0.54, "#6D28D9");
    gradient.addColorStop(1, "#14532D");
  } else if (world.phase === "morning") {
    gradient.addColorStop(0, "#0EA5E9");
    gradient.addColorStop(0.52, "#F59E0B");
    gradient.addColorStop(1, "#166534");
  } else {
    gradient.addColorStop(0, "#38BDF8");
    gradient.addColorStop(0.58, "#93C5FD");
    gradient.addColorStop(1, "#15803D");
  }
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, width, height);

  for (let i = 0; i < 52; i += 1) {
    const sx = (i * 47 + frame * 0.08) % width;
    const sy = (i * 31) % (height * 0.58);
    const alpha = world.phase === "night" ? 0.72 : 0.22;
    fillPixelRect(
      ctx,
      sx,
      sy,
      i % 5 === 0 ? 3 : 2,
      i % 7 === 0 ? 3 : 2,
      `rgba(255,255,255,${alpha})`,
    );
  }

  drawCelestial(ctx, world, frame, width, height);
  drawWeather(ctx, world, frame, width, height);

  fillPixelRect(
    ctx,
    0,
    height * 0.74,
    width,
    height * 0.26,
    "rgba(20,83,45,0.86)",
  );
  fillPixelRect(ctx, 0, height * 0.8, width, 6, "rgba(134,239,172,0.5)");
  for (let i = 0; i < width; i += 18) {
    const grassHeight = 4 + ((i + frame) % 9);
    fillPixelRect(
      ctx,
      i,
      height * 0.79 - grassHeight,
      3,
      grassHeight,
      "rgba(187,247,208,0.7)",
    );
  }

  for (const item of world.objects) {
    drawObject(ctx, item, frame, width, height);
  }
}

function weatherEmoji(world: BuddyWorldState): string {
  switch (world.weather) {
    case "storm":
      return "⛈️";
    case "rain":
      return "🌧️";
    case "wind":
      return "🍃";
    case "busy":
      return "🌀";
    case "dream":
      return "💤";
    case "aurora":
      return "🌌";
    case "clear":
      return "🌤️";
  }
}

export const BuddyWorld: React.FC<BuddyWorldProps> = ({
  palette,
  stage,
  state,
  pulse,
  pet,
  nowPlaying,
  activeQuest,
  activeSpeech,
  setupNeeded,
  compact = false,
  onCanvasEvent,
  onCare,
  onOpenPage,
  onRunMode,
  onDismissSetup,
  onSpeechControl,
  now,
}) => {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [currentTime, setCurrentTime] = useState(() => now ?? new Date());
  const [reaction, setReaction] = useState<string | null>(null);

  useEffect(() => {
    if (now) {
      setCurrentTime(now);
      return;
    }
    const timer = window.setInterval(() => setCurrentTime(new Date()), 60_000);
    return () => window.clearInterval(timer);
  }, [now]);

  useEffect(() => {
    if (!reaction) return;
    const timer = window.setTimeout(() => setReaction(null), 5000);
    return () => window.clearTimeout(timer);
  }, [reaction]);

  const world = useMemo(
    () =>
      buildBuddyWorldState({
        now: currentTime,
        pulse,
        pet,
        nowPlaying,
        activeQuest,
      }),
    [activeQuest, currentTime, nowPlaying, pet, pulse],
  );

  useEffect(() => {
    let frame = 0;
    let raf = 0;
    const render = () => {
      frame += 1;
      const canvas = canvasRef.current;
      const ctx = canvas?.getContext("2d");
      if (canvas && ctx) {
        const rect = canvas.getBoundingClientRect();
        const cssWidth = Math.max(1, Math.round(rect.width || 720));
        const cssHeight = Math.max(
          1,
          Math.round(rect.height || (compact ? 190 : 260)),
        );
        const ratio = window.devicePixelRatio || 1;
        const targetWidth = Math.round(cssWidth * ratio);
        const targetHeight = Math.round(cssHeight * ratio);
        if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
          canvas.width = targetWidth;
          canvas.height = targetHeight;
        }
        ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
        drawScene(ctx, world, frame, cssWidth, cssHeight);
      }
      raf = window.requestAnimationFrame(render);
    };
    render();
    return () => window.cancelAnimationFrame(raf);
  }, [compact, world]);

  const handleCelestialClick = () => {
    if (world.phase === "night") {
      onCare("sleep");
      setReaction("Buddy curls up under the moon and saves energy.");
      return;
    }
    onCare("play", "scroll");
    setReaction("Buddy catches a warm sunbeam and opens the focus scroll.");
  };

  const handleWeatherClick = () => {
    if (world.weather === "storm") {
      onOpenPage({ type: "stats" });
      setReaction("Buddy marked the storm front for investigation.");
      return;
    }
    if (world.weather === "rain") {
      onOpenPage({ type: "knowledge_graph" });
      setReaction("Buddy follows the rain into the memory garden.");
      return;
    }
    onCare("pet");
    setReaction("Buddy chirps back at the sky.");
  };

  const speechOverride = activeSpeech ? activeSpeech.text : null;
  const statusLine = reaction;

  return (
    <section
      className={classNames(styles.scene, { [styles.compact]: compact })}
      data-phase={world.phase}
      data-weather={world.weather}
      data-testid="buddy-world"
      aria-label="Buddy virtual scene"
    >
      <canvas
        ref={canvasRef}
        className={styles.canvas}
        data-testid="buddy-world-canvas"
      />

      <div className={styles.sceneHud}>
        <div className={styles.titleGroup}>
          <Text size="1" color="gray" className={styles.sectionLabel}>
            Buddy scene
          </Text>
          <Text size="3" weight="bold">
            {world.phaseLabel}
          </Text>
        </div>
        <div
          className={classNames(
            styles.vitalityBadge,
            VITALITY_CLASS[world.vitality],
          )}
        >
          {world.vitalityLabel}
        </div>
      </div>

      <button
        type="button"
        className={classNames(styles.hotspot, styles.celestialHotspot)}
        style={{ left: `${world.celestialX}%`, top: `${world.celestialY}%` }}
        onClick={handleCelestialClick}
        aria-label={`${world.celestialAction} with ${world.celestialLabel}`}
        title={`${world.celestialAction} with ${world.celestialLabel}`}
      >
        <span>{world.celestialEmoji}</span>
      </button>

      <button
        type="button"
        className={classNames(styles.hotspot, styles.weatherHotspot)}
        style={{ left: `${world.weatherX}%`, top: `${world.weatherY}%` }}
        onClick={handleWeatherClick}
        aria-label={`Interact with ${world.weatherLabel}`}
        title={world.weatherLabel}
      >
        <span>{weatherEmoji(world)}</span>
      </button>

      {world.objects.map((item) => (
        <button
          key={item.id}
          type="button"
          className={classNames(styles.objectHotspot, TONE_CLASS[item.tone])}
          style={{ left: `${item.x}%`, top: `${item.y}%` }}
          onClick={() => {
            onOpenPage(item.page);
            setReaction(`Buddy hops toward ${item.label.toLowerCase()}.`);
          }}
          aria-label={`Open ${item.label}`}
          title={`${item.label}: ${item.description}`}
        >
          <span className={styles.hotspotDot} />
          <span className={styles.objectTooltip}>
            <span className={styles.objectLabel}>{item.label}</span>
            <span className={styles.objectValue}>{item.value}</span>
          </span>
        </button>
      ))}

      <BuddyCharacter
        state={state}
        stage={stage}
        palette={palette}
        displaySize={compact ? 150 : 205}
        speechText={speechOverride}
        speechControls={activeSpeech ? activeSpeech.controls : undefined}
        onCanvasEvent={onCanvasEvent}
        onSpeechControl={activeSpeech ? onSpeechControl : undefined}
      />

      {setupNeeded && (
        <div className={styles.setupDock}>
          {SETUP_MODE_ACTIONS.map((item) => (
            <button
              key={item.mode}
              type="button"
              className={styles.sceneButton}
              onClick={() => onRunMode(item.mode)}
            >
              {item.label}
            </button>
          ))}
          <button
            type="button"
            className={styles.sceneButtonGhost}
            onClick={onDismissSetup}
          >
            Later
          </button>
        </div>
      )}

      <div className={styles.careDock} aria-label="Buddy scene care actions">
        <button
          type="button"
          className={styles.sceneButton}
          onClick={() => onCare("feed")}
        >
          🍜 Water
        </button>
        <button
          type="button"
          className={styles.sceneButton}
          onClick={() => onCare("play", "bug")}
        >
          🐛 Hunt
        </button>
        <button
          type="button"
          className={styles.sceneButton}
          onClick={() => onCare("clean")}
        >
          🧼 Clean
        </button>
        <button
          type="button"
          className={styles.sceneButton}
          onClick={() => onCare("sleep")}
        >
          😴 Rest
        </button>
      </div>

      {statusLine && (
        <div className={styles.caption}>
          <Text size="2">{statusLine}</Text>
        </div>
      )}
    </section>
  );
};
