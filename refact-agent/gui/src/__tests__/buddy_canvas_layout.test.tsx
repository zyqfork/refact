import { render, screen, waitFor } from "../utils/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BuddyCanvas } from "../features/Buddy/BuddyCanvas";
import type { BuddySemanticState } from "../features/Buddy/types";

const noopCanvasContext = {
  clearRect: vi.fn(),
  fillRect: vi.fn(),
  fillText: vi.fn(),
  getImageData: vi.fn(() => ({ data: new Uint8ClampedArray(4) }) as ImageData),
  putImageData: vi.fn(),
  restore: vi.fn(),
  save: vi.fn(),
  scale: vi.fn(),
  translate: vi.fn(),
  imageSmoothingEnabled: false,
  globalAlpha: 1,
  fillStyle: "#000000",
  font: "",
  textAlign: "center" as CanvasTextAlign,
  textBaseline: "top" as CanvasTextBaseline,
} satisfies Partial<CanvasRenderingContext2D>;

const noopContext = noopCanvasContext as unknown as CanvasRenderingContext2D;

function makeSemanticState(): BuddySemanticState {
  return {
    name: "Buddy",
    paletteIndex: 0,
    born: 0,
    mood: {
      happiness: 80,
      energy: 80,
      curiosity: 70,
      anxiety: 0,
      boredom: 10,
      affection: 80,
    },
    personality: {
      playfulness: 70,
      confidence: 60,
      clinginess: 70,
      resilience: 60,
      chaos: 30,
      sociability: 70,
      curiosity: 70,
    },
    progress: { xp: 0, stage: 2 },
    activity: {
      mood: "idle",
      animationType: "idle",
      lastSignalTime: 0,
      lastSignalType: null,
    },
    skills: [],
    log: [],
  };
}

describe("BuddyCanvas compact speech layout", () => {
  let frameCallbacks: FrameRequestCallback[] = [];

  beforeEach(() => {
    frameCallbacks = [];
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frameCallbacks.push(callback);
      return frameCallbacks.length;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {
      return undefined;
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function runAnimationFrame(): void {
    const callback = frameCallbacks.shift();
    if (callback) callback(0);
  }

  it("caps long compact world speech with real bubble rendering", async () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      noopContext,
    );

    render(
      <BuddyCanvas
        state={makeSemanticState()}
        displaySize={230}
        speechOverride="The observatory has a very long update that should avoid narrow side clipping in compact scenes."
        bubblePosition="top"
        compactBubble
      />,
    );

    runAnimationFrame();

    const bubble = await screen.findByText(/observatory has a very long/u);
    const bubbleElement = bubble.closest("div[data-bubble-position]");

    expect(bubbleElement).not.toBeNull();
    await waitFor(() => {
      expect(bubbleElement).toHaveStyle({
        width: "220px",
        maxWidth: "220px",
        whiteSpace: "normal",
      });
      expect(bubbleElement).toHaveAttribute("data-bubble-position", "top");
    });
  });

  it("keeps normal world side bubble sizing at the same display size", async () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(
      noopContext,
    );

    render(
      <BuddyCanvas
        state={makeSemanticState()}
        displaySize={230}
        speechOverride="The observatory has a very long update that should stay side aware in normal scenes."
        bubblePosition="left"
      />,
    );

    runAnimationFrame();

    const bubble = await screen.findByText(/observatory has a very long/u);
    const bubbleElement = bubble.closest("div[data-bubble-position]");

    expect(bubbleElement).not.toBeNull();
    await waitFor(() => {
      expect(bubbleElement).toHaveStyle({
        width: "270px",
        maxWidth: "300px",
      });
      expect(bubbleElement).toHaveAttribute("data-bubble-position", "left");
    });
  });
});
