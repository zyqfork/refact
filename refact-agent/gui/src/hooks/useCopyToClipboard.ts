import { useCallback } from "react";
import { fallbackCopying } from "../utils/fallbackCopying";

export const useCopyToClipboard = () => {
  return useCallback((text: string) => {
    void navigator.clipboard.writeText(text).catch(() => {
      fallbackCopying(text);
    });
  }, []);
};
