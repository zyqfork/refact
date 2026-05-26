import React, { useMemo, useState, useCallback } from "react";
import { Pencil1Icon, PlusIcon } from "@radix-ui/react-icons";
import { Flex, Text, Box, Spinner, Button } from "@radix-ui/themes";
import { ToolCard, ToolStatus } from "./ToolCard";
import { useStoredOpen } from "../useStoredOpen";
import { useAppSelector, useEventsBusForIDE } from "../../../hooks";
import {
  selectManyDiffMessageByIds,
  selectIsStreaming,
  selectIsWaiting,
  selectToolResultById,
} from "../../../features/Chat/Thread/selectors";
import { selectChatId, selectCanPaste } from "../../../features/Chat";
import { ToolCall, DiffChunk } from "../../../services/refact/types";
import { toolsApi } from "../../../services/refact";
import {
  parseRawTextDocToolCall,
  isRawTextDocToolCall,
  isCreateTextDocToolCall,
  isUpdateTextDocToolCall,
  isUpdateTextDocByLinesToolCall,
} from "../../Tools/types";
import { basename } from "./utils";
import styles from "./EditTool.module.css";

interface EditToolProps {
  toolCall: ToolCall;
  diffs?: DiffChunk[];
  isActiveTool?: boolean;
}

function countNonEmptyLines(text: string): number {
  let count = 0;
  let hasContent = false;

  for (const char of text) {
    if (char === "\n") {
      if (hasContent) count++;
      hasContent = false;
    } else if (char !== "\r" && char !== " " && char !== "\t") {
      hasContent = true;
    }
  }

  return hasContent ? count + 1 : count;
}

function getDiffStats(diffs: DiffChunk[]): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const diff of diffs) {
    added += countNonEmptyLines(diff.lines_add);
    removed += countNonEmptyLines(diff.lines_remove);
  }
  return { added, removed };
}

function getFilePath(toolCall: ToolCall): string | null {
  try {
    const args = JSON.parse(toolCall.function.arguments) as Record<
      string,
      unknown
    >;
    return typeof args.path === "string" ? args.path : null;
  } catch {
    return null;
  }
}

function isCreateTool(name: string | undefined): boolean {
  return name === "create_textdoc";
}

function handleKeyboardClick(
  event: React.KeyboardEvent,
  action: () => void,
): void {
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  event.stopPropagation();
  action();
}

const CONTEXT_LINES = 1;
const MAX_VISIBLE_DIFF_LINES = 80;

type DiffLineKind = "context" | "remove" | "add";

type RenderedDiffLine = {
  kind: DiffLineKind;
  oldLineNumber?: number;
  newLineNumber?: number;
  sign: string;
  line: string;
};

type RenderedDiffHunk = {
  header: string;
  lines: RenderedDiffLine[];
};

function displayLineNumber(line: RenderedDiffLine): number | undefined {
  if (line.kind === "add") return line.newLineNumber;
  if (line.kind === "context") {
    if (line.oldLineNumber === undefined) return line.newLineNumber;
    if (line.newLineNumber === undefined) return line.oldLineNumber;
    return contextLineNumber(line.oldLineNumber, line.newLineNumber);
  }
  return line.oldLineNumber;
}

const DiffLine: React.FC<RenderedDiffLine> = (line) => {
  const { kind, sign } = line;
  const rowClass =
    kind === "remove"
      ? styles.remove
      : kind === "add"
        ? styles.add
        : styles.context;
  return (
    <div className={`${styles.diffLine} ${rowClass}`}>
      <span className={styles.lineNumber}>{displayLineNumber(line) ?? ""}</span>
      <span className={styles.sign}>{sign}</span>
      <span className={styles.lineContent}>{line.line}</span>
    </div>
  );
};

function splitDiffLines(text: string): string[] {
  if (!text) return [];

  const normalized = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const lines = normalized.split("\n");
  if (normalized.endsWith("\n")) lines.pop();
  return lines;
}

function firstContextLine(text: string | null | undefined): string | null {
  const lines = splitDiffLines(text ?? "");
  return lines[0] ?? null;
}

function commonPrefixLength(left: string[], right: string[]): number {
  const max = Math.min(left.length, right.length);
  let count = 0;
  while (count < max && left[count] === right[count]) count++;
  return count;
}

function commonSuffixLength(
  left: string[],
  right: string[],
  prefixLength: number,
): number {
  const max = Math.min(left.length, right.length) - prefixLength;
  let count = 0;
  while (
    count < max &&
    left[left.length - 1 - count] === right[right.length - 1 - count]
  ) {
    count++;
  }
  return count;
}

function lineSpan(start: number, count: number): string {
  return count === 1 ? String(start) : `${start},${count}`;
}

function formatHunkHeader(
  diff: DiffChunk,
  oldLineCount: number,
  newLineCount: number,
): string {
  return `@@ -${lineSpan(diff.line1, oldLineCount)} +${lineSpan(
    diff.line2,
    newLineCount,
  )} @@`;
}

function contextLineNumber(
  oldLineNumber: number,
  newLineNumber: number,
): number {
  return oldLineNumber === newLineNumber ? oldLineNumber : newLineNumber;
}

function buildRenderedHunk(diff: DiffChunk): RenderedDiffHunk {
  const removeLines = splitDiffLines(diff.lines_remove);
  const addLines = splitDiffLines(diff.lines_add);
  const prefixLength = commonPrefixLength(removeLines, addLines);
  const suffixLength = commonSuffixLength(removeLines, addLines, prefixLength);
  const lines: RenderedDiffLine[] = [];
  const backendBeforeLine = firstContextLine(diff.lines_before);
  if (backendBeforeLine !== null) {
    lines.push({
      kind: "context",
      oldLineNumber: diff.line1 > 1 ? diff.line1 - 1 : undefined,
      newLineNumber: diff.line2 > 1 ? diff.line2 - 1 : undefined,
      sign: " ",
      line: backendBeforeLine,
    });
  }

  const inferredBeforeContextLines =
    backendBeforeLine === null ? CONTEXT_LINES : 0;
  const beforeContextStart = Math.max(
    0,
    prefixLength - inferredBeforeContextLines,
  );
  for (let i = beforeContextStart; i < prefixLength; i++) {
    lines.push({
      kind: "context",
      oldLineNumber: diff.line1 + i,
      newLineNumber: diff.line2 + i,
      sign: " ",
      line: removeLines[i],
    });
  }

  const removeChangeEnd = removeLines.length - suffixLength;
  for (let i = prefixLength; i < removeChangeEnd; i++) {
    lines.push({
      kind: "remove",
      oldLineNumber: diff.line1 + i,
      sign: "-",
      line: removeLines[i],
    });
  }

  const addChangeEnd = addLines.length - suffixLength;
  for (let i = prefixLength; i < addChangeEnd; i++) {
    lines.push({
      kind: "add",
      newLineNumber: diff.line2 + i,
      sign: "+",
      line: addLines[i],
    });
  }

  const backendAfterLine = firstContextLine(diff.lines_after);
  const suffixStart = removeLines.length - suffixLength;
  const inferredAfterContextLines =
    backendAfterLine === null ? CONTEXT_LINES : 0;
  const afterContextEnd = Math.min(
    removeLines.length,
    suffixStart + inferredAfterContextLines,
  );
  for (let i = suffixStart; i < afterContextEnd; i++) {
    lines.push({
      kind: "context",
      oldLineNumber: diff.line1 + i,
      newLineNumber: diff.line2 + i,
      sign: " ",
      line: removeLines[i],
    });
  }

  if (backendAfterLine !== null) {
    lines.push({
      kind: "context",
      oldLineNumber: diff.line1 + removeLines.length,
      newLineNumber: diff.line2 + addLines.length,
      sign: " ",
      line: backendAfterLine,
    });
  }

  return {
    header: formatHunkHeader(diff, removeLines.length, addLines.length),
    lines,
  };
}

const DiffBlock: React.FC<{ diff: DiffChunk }> = ({ diff }) => {
  const [showAll, setShowAll] = useState(false);
  const hunk = useMemo(() => buildRenderedHunk(diff), [diff]);
  const isLarge = hunk.lines.length > MAX_VISIBLE_DIFF_LINES;
  const visibleLines = showAll
    ? hunk.lines
    : hunk.lines.slice(0, MAX_VISIBLE_DIFF_LINES);
  const hiddenLineCount = Math.max(0, hunk.lines.length - visibleLines.length);

  return (
    <Box className={styles.diffBlock}>
      <div className={styles.hunkHeader}>{hunk.header}</div>
      {visibleLines.map((line, i) => (
        <DiffLine
          key={`${line.kind}-${line.oldLineNumber ?? ""}-${
            line.newLineNumber ?? ""
          }-${i}`}
          {...line}
        />
      ))}
      {isLarge && (
        <button
          type="button"
          className={styles.showMoreButton}
          onClick={() => setShowAll((prev) => !prev)}
        >
          {showAll
            ? "Show fewer diff lines"
            : `Show ${hiddenLineCount} more diff lines`}
        </button>
      )}
    </Box>
  );
};

interface FileEditItemProps {
  fileName: string;
  diffs: DiffChunk[];
  onOpenFile: () => void;
}

const FileEditItem: React.FC<FileEditItemProps> = ({
  fileName,
  diffs,
  onOpenFile,
}) => {
  const [isOpen, setIsOpen] = useState(true);
  const stats = useMemo(() => getDiffStats(diffs), [diffs]);

  const handleToggle = useCallback(() => {
    setIsOpen((prev) => !prev);
  }, []);

  const handleOpenClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onOpenFile();
    },
    [onOpenFile],
  );

  const handleHeaderKeyDown = useCallback(
    (event: React.KeyboardEvent) => {
      handleKeyboardClick(event, handleToggle);
    },
    [handleToggle],
  );

  return (
    <div className={styles.fileItem}>
      <Flex
        className={styles.fileHeader}
        align="center"
        gap="2"
        onClick={handleToggle}
        onKeyDown={handleHeaderKeyDown}
        role="button"
        tabIndex={0}
        aria-expanded={isOpen}
      >
        <Text size="1" className={styles.filename} onClick={handleOpenClick}>
          {basename(fileName)}
        </Text>
        <Text size="1" className={styles.stats}>
          {stats.added > 0 && (
            <span className={styles.added}>+{stats.added}</span>
          )}
          {stats.removed > 0 && (
            <span className={styles.removed}>−{stats.removed}</span>
          )}
        </Text>
      </Flex>

      {isOpen && (
        <Box className={styles.diffContent}>
          {diffs.map((diff, i) => (
            <DiffBlock key={i} diff={diff} />
          ))}
        </Box>
      )}
    </div>
  );
};

export const EditTool: React.FC<EditToolProps> = ({
  toolCall,
  diffs = [],
  isActiveTool = true,
}) => {
  const storeKey = toolCall.id ? `tc:${toolCall.id}` : undefined;
  const [isOpen, handleToggle] = useStoredOpen(storeKey, true);
  const { queryPathThenOpenFile, diffPasteBack, sendToolCallToIde } =
    useEventsBusForIDE();
  const [requestDryRun, dryRunResult] = toolsApi.useDryRunForEditToolMutation();
  const isStreaming = useAppSelector(selectIsStreaming);
  const isWaiting = useAppSelector(selectIsWaiting);
  const canPaste = useAppSelector(selectCanPaste);
  const chatId = useAppSelector(selectChatId);

  const maybeResult = useAppSelector((state) =>
    selectToolResultById(state, toolCall.id),
  );

  const diffIds = useMemo(
    () => (toolCall.id ? [toolCall.id] : []),
    [toolCall.id],
  );
  const selectDiffs = useMemo(
    () => selectManyDiffMessageByIds(diffIds),
    [diffIds],
  );
  const toolDiffs = useAppSelector(selectDiffs);

  const hasResult = maybeResult !== undefined;
  const hasDiffs = diffs.length > 0 || toolDiffs.length > 0;
  const isToolBusy = isActiveTool && !hasResult && (isStreaming || isWaiting);
  const shouldRenderDiffs = hasDiffs && !isToolBusy;

  const allDiffs = useMemo(() => {
    if (!shouldRenderDiffs) return [];

    const fromProps = diffs;
    const fromStore = toolDiffs.flatMap((d) => d.content);
    return fromProps.length > 0 ? fromProps : fromStore;
  }, [diffs, shouldRenderDiffs, toolDiffs]);

  const parsedToolCall = useMemo(() => {
    if (!isRawTextDocToolCall(toolCall)) return null;
    return parseRawTextDocToolCall(toolCall);
  }, [toolCall]);

  const replaceContent = useMemo(() => {
    if (!parsedToolCall) return null;
    if (isCreateTextDocToolCall(parsedToolCall)) {
      return parsedToolCall.function.arguments.content;
    }
    if (isUpdateTextDocToolCall(parsedToolCall)) {
      return parsedToolCall.function.arguments.replacement;
    }
    if (isUpdateTextDocByLinesToolCall(parsedToolCall)) {
      return parsedToolCall.function.arguments.content;
    }
    return null;
  }, [parsedToolCall]);

  const handleApplyDiff = useCallback(() => {
    if (!parsedToolCall) return;
    requestDryRun({
      toolName: parsedToolCall.function.name,
      toolArgs: parsedToolCall.function.arguments,
    })
      .then((results) => {
        if (results.data) {
          sendToolCallToIde(parsedToolCall, results.data, chatId);
        }
      })
      .catch(() => {
        /* ignore */
      });
  }, [chatId, parsedToolCall, requestDryRun, sendToolCallToIde]);

  const handleReplace = useCallback(() => {
    if (replaceContent !== null) {
      diffPasteBack(replaceContent, chatId, toolCall.id);
    }
  }, [chatId, diffPasteBack, replaceContent, toolCall.id]);

  const filePath = useMemo(() => {
    const fromArgs = getFilePath(toolCall);
    if (fromArgs) return fromArgs;
    if (allDiffs.length > 0) return allDiffs[0].file_name;
    return null;
  }, [toolCall, allDiffs]);
  const isCreate = isCreateTool(toolCall.function.name);
  const stats = useMemo(() => getDiffStats(allDiffs), [allDiffs]);

  const filesByName = useMemo(() => {
    const grouped: Record<string, DiffChunk[]> = {};
    for (const diff of allDiffs) {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      grouped[diff.file_name] = (grouped[diff.file_name] || []).concat(diff);
    }
    return grouped;
  }, [allDiffs]);

  const fileNames = Object.keys(filesByName);
  const isSingleFile = fileNames.length <= 1;

  const handleFileClick = useCallback(
    (e: React.MouseEvent, path: string) => {
      e.stopPropagation();
      void queryPathThenOpenFile({ file_path: path });
    },
    [queryPathThenOpenFile],
  );

  const handleFileKeyDown = useCallback(
    (event: React.KeyboardEvent, path: string) => {
      handleKeyboardClick(event, () => {
        void queryPathThenOpenFile({ file_path: path });
      });
    },
    [queryPathThenOpenFile],
  );

  const status: ToolStatus = useMemo(() => {
    if (
      maybeResult &&
      typeof maybeResult === "object" &&
      "tool_failed" in maybeResult &&
      maybeResult.tool_failed
    ) {
      return "error";
    }
    if (isToolBusy) return "running";
    if (hasResult || hasDiffs) return "success";
    return "running";
  }, [hasDiffs, hasResult, isToolBusy, maybeResult]);

  const summary = useMemo(() => {
    const statsEl =
      stats.added > 0 || stats.removed > 0 ? (
        <span className={styles.statsInline}>
          {stats.added > 0 && (
            <span className={styles.added}>+{stats.added}</span>
          )}
          {stats.removed > 0 && (
            <span className={styles.removed}>−{stats.removed}</span>
          )}
        </span>
      ) : null;

    const verb = isCreate ? "Create" : "Edit";
    if (isSingleFile && filePath) {
      return (
        <>
          {verb}{" "}
          <span
            className={styles.filename}
            onClick={(e) => handleFileClick(e, filePath)}
            onKeyDown={(event) => handleFileKeyDown(event, filePath)}
            role="button"
            tabIndex={0}
            aria-label={`Open ${filePath}`}
          >
            {basename(filePath)}
          </span>
          {statsEl && <> {statsEl}</>}
        </>
      );
    }
    if (fileNames.length > 1) {
      return (
        <>
          {verb} {fileNames.length} files {statsEl}
        </>
      );
    }
    return (
      <>
        {verb} file {statsEl}
      </>
    );
  }, [
    isCreate,
    isSingleFile,
    filePath,
    fileNames.length,
    handleFileClick,
    handleFileKeyDown,
    stats.added,
    stats.removed,
  ]);

  const icon = isCreate ? <PlusIcon /> : <Pencil1Icon />;

  return (
    <ToolCard
      icon={icon}
      summary={summary}
      status={status}
      isOpen={isOpen}
      onToggle={handleToggle}
      toolCall={toolCall}
    >
      {maybeResult?.content && typeof maybeResult.content === "string" && (
        <Box
          className={
            status === "error" ? styles.errorContent : styles.resultContent
          }
        >
          <Text size="1" color={status === "error" ? "red" : undefined}>
            {maybeResult.content}
          </Text>
        </Box>
      )}
      {shouldRenderDiffs && (
        <>
          <Flex gap="2" className={styles.actionBar}>
            <Button
              size="1"
              variant="soft"
              onClick={handleApplyDiff}
              disabled={dryRunResult.isLoading || !parsedToolCall}
            >
              {dryRunResult.isLoading ? (
                <Spinner size="1" />
              ) : (
                <Flex as="span" align="center" gap="1">
                  <PlusIcon />
                  Diff
                </Flex>
              )}
            </Button>
            {replaceContent !== null && (
              <Button
                size="1"
                variant="soft"
                onClick={handleReplace}
                disabled={!canPaste}
              >
                <Flex as="span" align="center" gap="1">
                  <PlusIcon />
                  Replace
                </Flex>
              </Button>
            )}
          </Flex>
          {isSingleFile ? (
            <Box className={styles.diffContent}>
              {allDiffs.map((diff, i) => (
                <DiffBlock key={i} diff={diff} />
              ))}
            </Box>
          ) : (
            <Flex direction="column" gap="1" className={styles.fileList}>
              {fileNames.map((fileName) => (
                <FileEditItem
                  key={fileName}
                  fileName={fileName}
                  diffs={filesByName[fileName]}
                  onOpenFile={() =>
                    void queryPathThenOpenFile({ file_path: fileName })
                  }
                />
              ))}
            </Flex>
          )}
        </>
      )}
    </ToolCard>
  );
};

export default EditTool;
