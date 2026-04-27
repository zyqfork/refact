import { store } from "../../app/store";
import { postBuddyErrorRequest } from "../../services/refact/buddy";

const REPORT_DEDUPE_MS = 10000;
const REPORT_TRIM_LEN = 4000;

const recentReports = new Map<string, number>();

const SECRET_PATTERNS: Array<[RegExp, string]> = [
  [/Bearer\s+[^\s"'`]+/gi, "Bearer [REDACTED]"],
  [/sk-[A-Za-z0-9]{20,}/g, "[REDACTED_SK_TOKEN]"],
  [/\bghp_[A-Za-z0-9]{10,}\b/g, "[REDACTED_GH_TOKEN]"],
  [/\bglpat-[A-Za-z0-9_\-]{10,}\b/g, "[REDACTED_GL_TOKEN]"],
  [
    /\b(api[_-]?key|token|secret|password)\s*[:=]\s*[^\s,;]+/gi,
    "$1=[REDACTED]",
  ],
  [/(https?:\/\/[^\s?#]+)\?[^\s)\]]+/gi, "$1?[REDACTED]"],
  [/file:\/\/[^\s)\]]+/gi, "file://[REDACTED_PATH]"],
  [/[A-Za-z]:\\[^\s)\]]+/g, "[REDACTED_PATH]"],
  [/\/(?:Users|home)\/[^\s)\]]+/g, "[REDACTED_PATH]"],
];

export type BuddyFrontendErrorSource =
  | "window_error"
  | "unhandledrejection"
  | "react_error_boundary"
  | "artifact_iframe";

function clipText(text: string, maxLen: number): string {
  if (text.length <= maxLen) return text;
  return `${text.slice(0, maxLen - 1).trimEnd()}…`;
}

function errorToText(error: unknown): string {
  if (error instanceof Error) {
    return error.stack || error.message;
  }
  if (typeof error === "string") return error;
  if (typeof error === "object" && error !== null) {
    if ("message" in error && typeof error.message === "string") {
      return error.message;
    }
    try {
      return JSON.stringify(error);
    } catch {
      return String(error);
    }
  }
  return String(error);
}

export function redactBuddyFrontendErrorText(text: string): string {
  return SECRET_PATTERNS.reduce(
    (current, [pattern, replacement]) => current.replace(pattern, replacement),
    text,
  );
}

export function buildBuddyFrontendErrorDedupeKey(
  args: {
    source: BuddyFrontendErrorSource;
    sourceFile?: string;
    toolName?: string;
    chatId?: string;
  },
  normalized: string,
): string {
  return [
    args.source,
    args.sourceFile ?? "",
    args.toolName ?? "",
    args.chatId ?? "",
    normalized.slice(0, 240),
  ].join("|");
}

export function resetBuddyFrontendErrorReportCache(): void {
  recentReports.clear();
}

function shouldReport(key: string, now: number): boolean {
  const previous = recentReports.get(key);
  if (previous && now - previous < REPORT_DEDUPE_MS) {
    return false;
  }

  recentReports.set(key, now);
  for (const [entry, timestamp] of recentReports) {
    if (now - timestamp > REPORT_DEDUPE_MS) {
      recentReports.delete(entry);
    }
  }
  return true;
}

type BuddyFrontendReporterState = {
  config: {
    apiKey: string | null;
    lspPort: number;
  };
};

type BuddyFrontendErrorDeps = {
  getState: () => BuddyFrontendReporterState;
  post: typeof postBuddyErrorRequest;
  now: () => number;
};

const defaultDeps: BuddyFrontendErrorDeps = {
  getState: () => store.getState() as BuddyFrontendReporterState,
  post: postBuddyErrorRequest,
  now: () => Date.now(),
};

export async function reportBuddyFrontendError(
  args: {
    source: BuddyFrontendErrorSource;
    error: unknown;
    sourceFile?: string;
    toolName?: string;
    chatId?: string;
  },
  deps: BuddyFrontendErrorDeps = defaultDeps,
): Promise<void> {
  const state = deps.getState();
  const port = state.config.lspPort;
  if (!port) return;

  const apiKey = state.config.apiKey ?? undefined;
  const normalized = clipText(
    redactBuddyFrontendErrorText(errorToText(args.error)).trim(),
    REPORT_TRIM_LEN,
  );
  if (!normalized) return;

  const key = buildBuddyFrontendErrorDedupeKey(args, normalized);
  if (!shouldReport(key, deps.now())) return;

  try {
    await deps.post(port, apiKey, {
      error: `[frontend:${args.source}] ${normalized}`,
      source_file: args.sourceFile ?? `frontend/${args.source}`,
      tool_name: args.toolName ?? args.source,
      chat_id: args.chatId,
    });
  } catch {
    return;
  }
}
