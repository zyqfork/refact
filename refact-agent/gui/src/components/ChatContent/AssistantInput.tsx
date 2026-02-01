import React, { useCallback, useMemo, useState } from "react";
import { Markdown } from "../Markdown";

import {
  Container,
  Box,
  Flex,
  Text,
  Link,
  Card,
  IconButton,
} from "@radix-ui/themes";
import { CopyIcon, CornerTopRightIcon, TrashIcon } from "@radix-ui/react-icons";
import {
  ThinkingBlock,
  ToolCall,
  WebSearchCitation,
} from "../../services/refact";
import { ToolContent } from "./ToolsContent";
import { fallbackCopying } from "../../utils/fallbackCopying";
import { telemetryApi } from "../../services/refact/telemetry";
import { ReasoningContent } from "./ReasoningContent";

type ChatInputProps = {
  message: string | null;
  reasoningContent?: string | null;
  thinkingBlocks?: ThinkingBlock[] | null;
  toolCalls?: ToolCall[] | null;
  serverExecutedTools?: ToolCall[] | null;
  citations?: WebSearchCitation[] | null;
  messageId?: string;
  onBranch?: (messageId: string) => void;
  onDelete?: (messageId: string) => void;
};

export const AssistantInput: React.FC<ChatInputProps> = ({
  message,
  reasoningContent,
  thinkingBlocks,
  toolCalls,
  serverExecutedTools,
  citations,
  messageId,
  onBranch,
  onDelete,
}) => {
  const [isHovered, setIsHovered] = useState(false);
  const [sendTelemetryEvent] =
    telemetryApi.useLazySendTelemetryChatEventQuery();

  // Get unique server-executed tool names for display
  const serverToolNames = useMemo(() => {
    if (!serverExecutedTools || serverExecutedTools.length === 0) return [];
    const names = serverExecutedTools
      .map((tool) => tool.function.name)
      .filter((name): name is string => !!name);
    return [...new Set(names)];
  }, [serverExecutedTools]);

  const handleCopy = useCallback(
    (text: string) => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (window.navigator?.clipboard?.writeText) {
        void window.navigator.clipboard
          .writeText(text)
          .catch(() => {
            // eslint-disable-next-line no-console
            console.log("failed to copy to clipboard");
            void sendTelemetryEvent({
              scope: `codeBlockCopyToClipboard`,
              success: false,
              error_message:
                "window.navigator?.clipboard?.writeText: failed to copy to clipboard",
            });
          })
          .then(() => {
            void sendTelemetryEvent({
              scope: `codeBlockCopyToClipboard`,
              success: true,
              error_message: "",
            });
          });
      } else {
        fallbackCopying(text);
        void sendTelemetryEvent({
          scope: `codeBlockCopyToClipboard`,
          success: true,
          error_message: "",
        });
      }
    },
    [sendTelemetryEvent],
  );

  const combinedReasoning = useMemo(() => {
    if (reasoningContent) {
      return reasoningContent;
    }
    if (thinkingBlocks && thinkingBlocks.length > 0) {
      const thinkingText = thinkingBlocks
        .filter((block) => block.thinking)
        .map((block) => block.thinking)
        .join("\n\n");
      if (thinkingText) {
        return thinkingText;
      }
    }
    return null;
  }, [reasoningContent, thinkingBlocks]);

  const handleCopyMessage = useCallback(() => {
    if (message) {
      handleCopy(message);
    }
  }, [message, handleCopy]);

  const handleBranch = useCallback(() => {
    if (messageId && onBranch) {
      onBranch(messageId);
    }
  }, [messageId, onBranch]);

  const handleDelete = useCallback(() => {
    if (messageId && onDelete) {
      onDelete(messageId);
    }
  }, [messageId, onDelete]);

  return (
    <Container
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
    >
      <Flex
        justify="end"
        gap="1"
        style={{
          opacity: isHovered ? 1 : 0,
          visibility: isHovered ? "visible" : "hidden",
          transition: "opacity 0.15s, visibility 0.15s",
        }}
      >
        <IconButton
          title="Copy message"
          variant="soft"
          size="2"
          onClick={handleCopyMessage}
        >
          <CopyIcon width={15} height={15} />
        </IconButton>
        {onBranch && messageId && (
          <IconButton
            title="Branch from here"
            variant="soft"
            size="2"
            onClick={handleBranch}
          >
            <CornerTopRightIcon width={15} height={15} />
          </IconButton>
        )}
        {onDelete && messageId && (
          <IconButton
            title="Delete message"
            variant="soft"
            size="2"
            color="red"
            onClick={handleDelete}
          >
            <TrashIcon width={15} height={15} />
          </IconButton>
        )}
      </Flex>
      {combinedReasoning && (
        <ReasoningContent
          reasoningContent={combinedReasoning}
          onCopyClick={handleCopy}
        />
      )}
      {message && (
        <Box py="4">
          <Markdown canHaveInteractiveElements={true} onCopyClick={handleCopy}>
            {message}
          </Markdown>
        </Box>
      )}
      {/* Server-executed tools indicator with citations */}
      {(serverToolNames.length > 0 || (citations && citations.length > 0)) && (
        <Card my="3" style={{ backgroundColor: "var(--gray-a2)" }}>
          <Flex direction="column" gap="2" p="2">
            {serverToolNames.length > 0 && (
              <Flex gap="2" align="center">
                <Text size="2">☁️</Text>
                <Text size="2" color="gray">
                  {serverToolNames.join(", ")}
                </Text>
              </Flex>
            )}
            {citations && citations.length > 0 && (
              <Flex
                direction="column"
                gap="1"
                style={{ maxHeight: "150px", overflowY: "auto" }}
              >
                <Text size="1" weight="medium" color="gray">
                  Sources:
                </Text>
                {citations
                  .filter(
                    (citation, idx, arr) =>
                      arr.findIndex((c) => c.url === citation.url) === idx,
                  )
                  .filter((citation) => {
                    try {
                      const url = new URL(citation.url);
                      return (
                        url.protocol === "http:" || url.protocol === "https:"
                      );
                    } catch {
                      return false;
                    }
                  })
                  .map((citation, idx) => (
                    <Link
                      key={idx}
                      href={citation.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      size="1"
                    >
                      {citation.title}
                    </Link>
                  ))}
              </Flex>
            )}
          </Flex>
        </Card>
      )}
      {toolCalls && <ToolContent toolCalls={toolCalls} />}
    </Container>
  );
};
