import React, { useCallback, useMemo, useState } from "react";
import {
  Badge,
  Box,
  Button,
  Card,
  Flex,
  Spinner,
  Text,
} from "@radix-ui/themes";
import classNames from "classnames";
import type { TaskMemoryEntry } from "../../../services/refact/taskMemoriesApi";
import styles from "./MemoryInboxPanel.module.css";

const KIND_COLORS: Record<
  TaskMemoryEntry["kind"],
  "blue" | "green" | "amber" | "red" | "purple" | "gray"
> = {
  decision: "purple",
  spec: "blue",
  finding: "green",
  gotcha: "amber",
  risk: "red",
  handoff: "purple",
  progress: "blue",
  postmortem: "amber",
  brief: "green",
  freeform: "gray",
};

const MAX_COLLAPSED_TAGS = 3;
const PREVIEW_LENGTH = 180;

type MemoryCardProps = {
  memory: TaskMemoryEntry;
  onPin: (filename: string, pinned: boolean) => void | Promise<void>;
  onArchive: (filename: string) => void | Promise<void>;
  disabled?: boolean;
  pending?: boolean;
};

function buildPreview(content: string): string {
  const trimmed = content.trim();
  if (trimmed.length <= PREVIEW_LENGTH) return trimmed;
  return `${trimmed.slice(0, PREVIEW_LENGTH).trimEnd()}…`;
}

export const MemoryCard: React.FC<MemoryCardProps> = ({
  memory,
  onPin,
  onArchive,
  disabled = false,
  pending = false,
}) => {
  const [expanded, setExpanded] = useState(false);
  const [tagsExpanded, setTagsExpanded] = useState(false);

  const handlePin = useCallback(() => {
    void onPin(memory.filename, !memory.pinned);
  }, [memory.filename, memory.pinned, onPin]);

  const handleArchive = useCallback(() => {
    void onArchive(memory.filename);
  }, [memory.filename, onArchive]);

  const createdAt = memory.created_at_known
    ? new Date(memory.created_at).toLocaleString()
    : "unknown time";
  const title = memory.title.trim() || memory.filename;
  const content = memory.content.trim() || "No content";
  const preview = useMemo(() => buildPreview(content), [content]);
  const canExpand = preview !== content;
  const visibleTags = tagsExpanded
    ? memory.tags
    : memory.tags.slice(0, MAX_COLLAPSED_TAGS);
  const hiddenTagCount = memory.tags.length - visibleTags.length;

  return (
    <Card
      className={classNames(styles.card, memory.pinned && styles.cardPinned)}
      data-testid={`memory-card-${memory.filename}`}
    >
      <Flex direction="column" gap="2">
        <Flex
          justify="between"
          align="start"
          gap="2"
          className={styles.cardTopRow}
        >
          <Box className={styles.cardTitleBlock}>
            <Text weight="medium" size="2" className={styles.cardTitle}>
              {title}
            </Text>
            <Flex
              gap="1"
              align="center"
              wrap="wrap"
              className={styles.cardMeta}
            >
              <Badge color={KIND_COLORS[memory.kind]} variant="soft">
                {memory.kind}
              </Badge>
              {memory.pinned && (
                <Badge color="amber" variant="solid">
                  pinned
                </Badge>
              )}
              <Text size="1" color="gray" className={styles.cardMetaText}>
                {memory.namespace}
              </Text>
            </Flex>
          </Box>
          <Text size="1" color="gray" className={styles.cardDate}>
            {createdAt}
          </Text>
        </Flex>

        <Box
          className={classNames(
            styles.preview,
            expanded && styles.previewExpanded,
          )}
        >
          <Text as="div" size="2" color="gray">
            {expanded ? content : preview}
          </Text>
        </Box>

        {memory.tags.length > 0 && (
          <Flex gap="1" wrap="wrap" align="center" className={styles.tags}>
            {visibleTags.map((tag) => (
              <Badge key={tag} color="gray" variant="outline">
                {tag}
              </Badge>
            ))}
            {hiddenTagCount > 0 && (
              <Button
                type="button"
                size="1"
                variant="ghost"
                className={styles.tagsToggle}
                onClick={() => setTagsExpanded(true)}
              >
                Show {hiddenTagCount} more
              </Button>
            )}
            {tagsExpanded && memory.tags.length > MAX_COLLAPSED_TAGS && (
              <Button
                type="button"
                size="1"
                variant="ghost"
                className={styles.tagsToggle}
                onClick={() => setTagsExpanded(false)}
              >
                Show fewer
              </Button>
            )}
          </Flex>
        )}

        <Flex gap="2" wrap="wrap" align="center" className={styles.actions}>
          <Button
            size="1"
            variant="soft"
            onClick={handlePin}
            disabled={disabled}
          >
            {memory.pinned ? "Unpin" : "Pin"}
          </Button>
          <Button
            size="1"
            variant="soft"
            color="amber"
            onClick={handleArchive}
            disabled={disabled}
          >
            Archive
          </Button>
          {canExpand && (
            <Button
              size="1"
              variant="ghost"
              onClick={() => setExpanded((value) => !value)}
              aria-expanded={expanded}
            >
              {expanded ? "Collapse" : "Expand"}
            </Button>
          )}
          {pending && (
            <Flex align="center" gap="1" className={styles.pendingState}>
              <Spinner size="1" />
              <Text size="1" color="gray">
                Updating
              </Text>
            </Flex>
          )}
        </Flex>
      </Flex>
    </Card>
  );
};

export default MemoryCard;
