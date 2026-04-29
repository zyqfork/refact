import type {
  BuddyControl,
  BuddyRuntimeEvent,
  BuddySpeechItem,
  BuddySuggestion,
} from "./types";

export type BuddySceneSpeechSource = "speech" | "runtime" | "suggestion";

export interface BuddySceneSpeech {
  text: string;
  controls: BuddyControl[];
  chat_id?: string;
  source: BuddySceneSpeechSource;
  runtimeEventId?: string;
}

function runtimeEventText(event: BuddyRuntimeEvent): string {
  if (event.speech_text?.trim()) return event.speech_text;
  if (event.description?.trim() && event.status === "failed") {
    return `${event.title}: ${event.description}`;
  }
  return event.title;
}

function runtimeEventToSpeech(
  event: BuddyRuntimeEvent | null | undefined,
): BuddySceneSpeech | null {
  if (!event || event.dismissed) return null;
  const text = runtimeEventText(event).trim();
  if (!text) return null;
  return {
    text,
    controls: event.controls ?? [],
    chat_id: event.chat_id,
    source: "runtime",
    runtimeEventId: event.id,
  };
}

function suggestionToSpeech(
  suggestion: BuddySuggestion | null | undefined,
): BuddySceneSpeech | null {
  if (!suggestion || suggestion.dismissed) return null;
  return {
    text: `${suggestion.title}: ${suggestion.description}`,
    controls: suggestion.controls.map((control) =>
      control.action === "dismiss"
        ? {
            ...control,
            action: "dismiss_suggestion",
            action_param: suggestion.id,
          }
        : control,
    ),
    source: "suggestion",
  };
}

function runtimeFromQueue(
  nowPlaying: BuddyRuntimeEvent | null,
  runtimeQueue: BuddyRuntimeEvent[],
): BuddyRuntimeEvent | null {
  if (nowPlaying && !nowPlaying.dismissed) return nowPlaying;
  return runtimeQueue.find((event) => !event.dismissed) ?? null;
}

export function buildBuddySceneSpeech(args: {
  activeSpeech: BuddySpeechItem | null;
  nowPlaying: BuddyRuntimeEvent | null;
  runtimeQueue: BuddyRuntimeEvent[];
  activeSuggestion?: BuddySuggestion | null;
}): BuddySceneSpeech | null {
  if (args.activeSpeech) {
    return {
      text: args.activeSpeech.text,
      controls: args.activeSpeech.controls,
      chat_id: args.activeSpeech.chat_id,
      source: "speech",
    };
  }

  return (
    runtimeEventToSpeech(
      runtimeFromQueue(args.nowPlaying, args.runtimeQueue),
    ) ?? suggestionToSpeech(args.activeSuggestion)
  );
}
