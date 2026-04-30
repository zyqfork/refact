function textFromErrorData(data: unknown): string | null {
  if (typeof data === "string") return data;
  if (typeof data !== "object" || data === null) return null;
  if ("error" in data && typeof data.error === "string") return data.error;
  if ("detail" in data && typeof data.detail === "string") return data.detail;
  if ("message" in data && typeof data.message === "string") {
    return data.message;
  }
  return null;
}

export function worktreeErrorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  if (typeof error === "object" && error !== null) {
    if ("data" in error) {
      const dataText = textFromErrorData(error.data);
      if (dataText) return dataText;
    }
    const directText = textFromErrorData(error);
    if (directText) return directText;
  }
  return String(error);
}
