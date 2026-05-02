import { describe, expect, it, vi } from "vitest";
import { drawBuddyWorld } from "../features/Buddy/buddyWorldDraw";
import {
  buildBuddyWorldState,
  type BuddyWorldState,
} from "../features/Buddy/buddyWorldModel";
import { PALETTES } from "../features/Buddy/constants";
import type { BuddyPetState, BuddyPulse } from "../features/Buddy/types";

type MockCanvasContext = Pick<
  CanvasRenderingContext2D,
  | "arc"
  | "beginPath"
  | "bezierCurveTo"
  | "clearRect"
  | "closePath"
  | "createLinearGradient"
  | "ellipse"
  | "fill"
  | "fillRect"
  | "fillText"
  | "lineTo"
  | "moveTo"
  | "restore"
  | "save"
  | "stroke"
> &
  Partial<CanvasRenderingContext2D>;

type RecordedCanvasContext = CanvasRenderingContext2D & {
  alphaWrites: number[];
  drawOps: string[];
};

function makeCanvasContext(): RecordedCanvasContext {
  const gradientStops: string[] = [];
  const gradient = {
    addColorStop: vi.fn((offset: number, color: string) => {
      gradientStops.push(`stop:${offset.toFixed(3)}:${color}`);
    }),
  } as unknown as CanvasGradient;
  const alphaWrites: number[] = [];
  const drawOps: string[] = [];
  let globalAlphaValue = 1;
  let fillStyleValue: CanvasRenderingContext2D["fillStyle"] = "#000000";
  let strokeStyleValue: CanvasRenderingContext2D["strokeStyle"] = "#000000";
  const formatNumber = (value: number) => value.toFixed(3);
  const ctx: MockCanvasContext & {
    alphaWrites: number[];
    drawOps: string[];
  } = {
    alphaWrites,
    drawOps,
    arc: vi.fn(
      (
        x: number,
        y: number,
        radius: number,
        startAngle: number,
        endAngle: number,
      ) => {
        drawOps.push(
          `arc:${formatNumber(x)}:${formatNumber(y)}:${formatNumber(
            radius,
          )}:${formatNumber(startAngle)}:${formatNumber(endAngle)}`,
        );
      },
    ),
    beginPath: vi.fn(() => drawOps.push("beginPath")),
    bezierCurveTo: vi.fn(
      (
        cp1x: number,
        cp1y: number,
        cp2x: number,
        cp2y: number,
        x: number,
        y: number,
      ) => {
        drawOps.push(
          `bezierCurveTo:${formatNumber(cp1x)}:${formatNumber(
            cp1y,
          )}:${formatNumber(cp2x)}:${formatNumber(cp2y)}:${formatNumber(
            x,
          )}:${formatNumber(y)}`,
        );
      },
    ),
    clearRect: vi.fn((x: number, y: number, width: number, height: number) => {
      drawOps.push(
        `clearRect:${formatNumber(x)}:${formatNumber(y)}:${formatNumber(
          width,
        )}:${formatNumber(height)}`,
      );
    }),
    closePath: vi.fn(() => drawOps.push("closePath")),
    createLinearGradient: vi.fn(
      (x0: number, y0: number, x1: number, y1: number) => {
        drawOps.push(
          `createLinearGradient:${formatNumber(x0)}:${formatNumber(
            y0,
          )}:${formatNumber(x1)}:${formatNumber(y1)}`,
        );
        return gradient;
      },
    ),
    ellipse: vi.fn(
      (
        x: number,
        y: number,
        radiusX: number,
        radiusY: number,
        rotation: number,
        startAngle: number,
        endAngle: number,
      ) => {
        drawOps.push(
          `ellipse:${formatNumber(x)}:${formatNumber(y)}:${formatNumber(
            radiusX,
          )}:${formatNumber(radiusY)}:${formatNumber(rotation)}:${formatNumber(
            startAngle,
          )}:${formatNumber(endAngle)}`,
        );
      },
    ),
    fill: vi.fn(() =>
      drawOps.push(`fill:${String(fillStyleValue)}:${globalAlphaValue}`),
    ),
    fillRect: vi.fn((x: number, y: number, width: number, height: number) => {
      drawOps.push(
        `fillRect:${formatNumber(x)}:${formatNumber(y)}:${formatNumber(
          width,
        )}:${formatNumber(height)}:${String(
          fillStyleValue,
        )}:${globalAlphaValue}`,
      );
    }),
    fillText: vi.fn((text: string, x: number, y: number) => {
      drawOps.push(
        `fillText:${text}:${formatNumber(x)}:${formatNumber(y)}:${String(
          fillStyleValue,
        )}:${globalAlphaValue}`,
      );
    }),
    lineTo: vi.fn((x: number, y: number) => {
      drawOps.push(`lineTo:${formatNumber(x)}:${formatNumber(y)}`);
    }),
    moveTo: vi.fn((x: number, y: number) => {
      drawOps.push(`moveTo:${formatNumber(x)}:${formatNumber(y)}`);
    }),
    restore: vi.fn(() => drawOps.push("restore")),
    save: vi.fn(() => drawOps.push("save")),
    stroke: vi.fn(() =>
      drawOps.push(`stroke:${String(strokeStyleValue)}:${globalAlphaValue}`),
    ),
    font: "10px monospace",
    get fillStyle() {
      return fillStyleValue;
    },
    set fillStyle(value: CanvasRenderingContext2D["fillStyle"]) {
      fillStyleValue = value === gradient ? gradientStops.join("|") : value;
    },
    get globalAlpha() {
      return globalAlphaValue;
    },
    set globalAlpha(value: number) {
      alphaWrites.push(value);
      globalAlphaValue = value;
    },
    imageSmoothingEnabled: false,
    lineCap: "round" as CanvasLineCap,
    lineWidth: 1,
    get strokeStyle() {
      return strokeStyleValue;
    },
    set strokeStyle(value: CanvasRenderingContext2D["strokeStyle"]) {
      strokeStyleValue = value;
    },
    textAlign: "center" as CanvasTextAlign,
    textBaseline: "middle" as CanvasTextBaseline,
  };
  return ctx as RecordedCanvasContext;
}

function makePet(overrides?: Partial<BuddyPetState>): BuddyPetState {
  return {
    needs: {
      hunger: 80,
      energy: 80,
      hygiene: 80,
      boredom: 10,
      affection: 80,
    },
    condition: {
      sleeping: false,
      hungry: false,
      sleepy: false,
      dirty: false,
      bored: false,
      lonely: false,
    },
    evolution: {
      care_score: 0,
      neglect_score: 0,
      open_seconds: 0,
      last_evolved_at: null,
    },
    ...overrides,
  };
}

function makePulse(overrides?: Partial<BuddyPulse>): BuddyPulse {
  const pulse: BuddyPulse = {
    generated_at: "2024-01-01T00:00:00Z",
    tasks: { total: 3, stuck: 0, abandoned: 0, by_status: {} },
    trajectories: { total: 10, untitled: 0, oldest_age_days: 1 },
    memory: { total: 5, orphan: 0, stale_conflicts: 0 },
    providers: { defaults_ok: true, broken_refs: 0, quota_warnings: 0 },
    mcp: { total: 4, failing: 0, auth_expiring: 0 },
    customization: { modes: 3, skills: 2, commands: 1, subagents: 0, hooks: 0 },
    diagnostics: { last_hour: 0, top_error_types: [] },
    git: { uncommitted_files: 0, diff_lines_4h: 0, branches: 3 },
    worktrees: {
      total_registered: 3,
      total_discovered: 1,
      total: 4,
      clean: 2,
      dirty: 1,
      unknown: 0,
      stale: 1,
      conflicted: 0,
      shared: 1,
      abandoned_clean: 2,
      changed_files: 3,
      additions: 10,
      deletions: 2,
      missing_registry_paths: 1,
      unregistered_cache_dirs: 1,
      merged_branches: 2,
    },
  };
  return { ...pulse, ...overrides };
}

function makeWorld(args?: {
  now?: Date;
  pulse?: BuddyPulse | null;
  pet?: BuddyPetState;
}): BuddyWorldState {
  return buildBuddyWorldState({
    now: args?.now ?? new Date("2024-01-01T14:00:00"),
    pulse: args?.pulse ?? makePulse(),
    pet: args?.pet ?? makePet(),
    nowPlaying: null,
    activeQuest: null,
  });
}

function drawWorld(
  world: BuddyWorldState,
  ctx = makeCanvasContext(),
): RecordedCanvasContext {
  drawBuddyWorld({
    ctx,
    world,
    palette: PALETTES[0],
    frame: 120,
    width: 720,
    height: 260,
    compact: false,
    reducedMotion: false,
  });
  return ctx;
}

function expectAlphaWritesClamped(ctx: RecordedCanvasContext): void {
  expect(ctx.alphaWrites.length).toBeGreaterThan(0);
  expect(ctx.alphaWrites.every((alpha) => alpha >= 0 && alpha <= 1)).toBe(true);
}

function expectDrawOpsFinite(ctx: RecordedCanvasContext): void {
  const serializedOps = ctx.drawOps.join(":");
  const numericTokens = ctx.drawOps.flatMap((operation) =>
    operation
      .split(":")
      .map((token) => Number(token))
      .filter((value) => !Number.isNaN(value)),
  );
  expect(serializedOps).not.toMatch(/\b(?:NaN|Infinity|-Infinity)\b/);
  expect(numericTokens.length).toBeGreaterThan(0);
  expect(numericTokens.every((value) => Number.isFinite(value))).toBe(true);
}

function expectHealthyDraw(ctx: RecordedCanvasContext): void {
  expectAlphaWritesClamped(ctx);
  expectDrawOpsFinite(ctx);
}

interface PaletteCase {
  label: string;
  now: Date;
  pet?: BuddyPetState;
  pulse?: BuddyPulse;
}

describe("drawBuddyWorld", () => {
  it.each<PaletteCase>([
    { label: "morning", now: new Date("2024-01-01T08:00:00") },
    { label: "day", now: new Date("2024-01-01T14:00:00") },
    { label: "evening", now: new Date("2024-01-01T18:00:00") },
    { label: "night", now: new Date("2024-01-01T23:00:00") },
    {
      label: "dream",
      now: new Date("2024-01-01T23:00:00"),
      pet: makePet({ condition: { ...makePet().condition, sleeping: true } }),
    },
    {
      label: "storm",
      now: new Date("2024-01-01T14:00:00"),
      pet: makePet(),
      pulse: makePulse({
        providers: { defaults_ok: true, broken_refs: 1, quota_warnings: 0 },
      }),
    },
  ])(
    "draws the $label palette hint without throwing",
    ({ now, pet, pulse }) => {
      const world = makeWorld({ now, pet, pulse });
      const ctx = drawWorld(world);

      expectHealthyDraw(ctx);
    },
  );

  it("draws all supported atmosphere layers without throwing", () => {
    const baseWorld = makeWorld();
    const world: BuddyWorldState = {
      ...baseWorld,
      weather: "aurora",
      atmosphere: {
        phase: baseWorld.phase,
        mood: "busy",
        primaryWeather: "aurora",
        layers: [
          "sun_motes",
          "moths",
          "fireflies",
          "stars",
          "aurora",
          "dream_mist",
          "workshop_runes",
          "provider_storm",
          "provider_flicker",
          "memory_orbs",
          "cozy_home_glow",
          "toy_glow",
          "empty_food_nook",
        ],
        intensity: 0.86,
        paletteHint: "storm",
        serious: true,
      },
    };
    const ctx = drawWorld(world);

    expectHealthyDraw(ctx);
  });

  it("draws compact reduced-motion mode without throwing", () => {
    const ctx = makeCanvasContext();

    drawBuddyWorld({
      ctx,
      world: makeWorld({ now: new Date("2024-01-01T23:00:00") }),
      palette: PALETTES[0],
      frame: 4,
      width: 360,
      height: 190,
      compact: true,
      reducedMotion: true,
    });

    expectHealthyDraw(ctx);
  });

  it("keeps storm, dream mist, memory orbs, and workshop runes finite with edge inputs", () => {
    const baseWorld = makeWorld({
      pulse: makePulse({
        providers: { defaults_ok: false, broken_refs: 2, quota_warnings: 3 },
      }),
    });
    const world: BuddyWorldState = {
      ...baseWorld,
      celestialX: Number.POSITIVE_INFINITY,
      celestialY: Number.NaN,
      weatherX: Number.NEGATIVE_INFINITY,
      weatherY: Number.NaN,
      atmosphere: {
        ...baseWorld.atmosphere,
        layers: [
          "provider_storm",
          "dream_mist",
          "memory_orbs",
          "workshop_runes",
        ],
        intensity: Number.POSITIVE_INFINITY,
        paletteHint: "storm",
      },
      objects: baseWorld.objects.map((item, index) => ({
        ...item,
        x: index % 2 === 0 ? Number.NaN : item.x,
        y: index % 2 === 1 ? Number.POSITIVE_INFINITY : item.y,
        size: Number.POSITIVE_INFINITY,
        intensity: Number.NaN,
        interactionX: Number.NEGATIVE_INFINITY,
        interactionY: Number.NaN,
        depthScale: Number.POSITIVE_INFINITY,
      })),
    };
    const ctx = makeCanvasContext();

    drawBuddyWorld({
      ctx,
      world,
      palette: PALETTES[0],
      frame: Number.POSITIVE_INFINITY,
      width: Number.POSITIVE_INFINITY,
      height: Number.NaN,
      compact: false,
      reducedMotion: false,
    });

    expectHealthyDraw(ctx);
  });

  it("draw output is deterministic for the same world and frame", () => {
    const world = makeWorld({
      now: new Date("2024-01-01T18:00:00"),
      pulse: makePulse({
        memory: { total: 12, orphan: 3, stale_conflicts: 1 },
        providers: { defaults_ok: false, broken_refs: 0, quota_warnings: 2 },
        diagnostics: { last_hour: 8, top_error_types: ["tool_failed"] },
      }),
    });
    const firstCtx = makeCanvasContext();
    const secondCtx = makeCanvasContext();
    const args = {
      world,
      palette: PALETTES[0],
      frame: 240,
      width: 720,
      height: 260,
      compact: false,
      reducedMotion: false,
    };

    drawBuddyWorld({ ctx: firstCtx, ...args });
    drawBuddyWorld({ ctx: secondCtx, ...args });

    expect(secondCtx.drawOps).toEqual(firstCtx.drawOps);
  });
});
