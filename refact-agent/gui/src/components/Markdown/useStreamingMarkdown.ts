import { useDeferredValue, useEffect, useState } from "react";

function scheduleDeferred(cb: () => void): () => void {
  if (typeof globalThis.requestAnimationFrame === "function") {
    const id = globalThis.requestAnimationFrame(() => cb());
    return () => globalThis.cancelAnimationFrame(id);
  }

  const id = setTimeout(cb, 16);
  return () => clearTimeout(id);
}

export function useStreamingMarkdown(
  text: string | null,
  isStreaming: boolean,
): string | null {
  const deferredText = useDeferredValue(text);
  const [mountedText, setMountedText] = useState<string | null>(
    isStreaming ? deferredText : text,
  );

  useEffect(() => {
    if (!isStreaming) {
      setMountedText(text);
      return;
    }

    let cancelled = false;
    const dispose = scheduleDeferred(() => {
      if (!cancelled) {
        setMountedText(deferredText);
      }
    });

    return () => {
      cancelled = true;
      dispose();
    };
  }, [deferredText, isStreaming, text]);

  return isStreaming ? mountedText : text;
}
