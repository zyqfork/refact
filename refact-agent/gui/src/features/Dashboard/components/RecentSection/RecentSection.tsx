import React, { useCallback, useMemo, useState } from "react";
import { Flex, IconButton, Skeleton, Spinner, Text, TextField, Tooltip } from "@radix-ui/themes";
import { MagnifyingGlassIcon, ChevronDownIcon, ChevronUpIcon, PlusIcon, CheckboxIcon } from "@radix-ui/react-icons";
import { Virtuoso } from "react-virtuoso";
import { useAppDispatch, useAppSelector, useLoadMoreHistory } from "../../../../hooks";
import {
  buildHistoryTree,
  ChatHistoryItem,
  deleteChatById,
  HistoryTreeNode,
  updateChatTitleById,
} from "../../../History/historySlice";
import { newChatAction, restoreChat } from "../../../Chat/Thread";
import { push } from "../../../Pages/pagesSlice";
import { useCreateTaskMutation } from "../../../../services/refact/tasks";
import { RecentItem, getDateGroup } from "./RecentItem";
import type { DashboardBreakpoint } from "../../types";
import styles from "./RecentSection.module.css";

type RecentSectionProps = {
  breakpoint: DashboardBreakpoint;
  expanded: boolean;
  onToggleExpand: () => void;
};

function treeMatchesQuery(node: HistoryTreeNode, query: string): boolean {
  if (
    node.title.toLowerCase().includes(query) ||
    (node.mode?.toLowerCase().includes(query) ?? false)
  ) {
    return true;
  }
  return node.children.some((child) => treeMatchesQuery(child, query));
}

export const RecentSection: React.FC<RecentSectionProps> = ({
  breakpoint,
  expanded,
  onToggleExpand,
}) => {
  const dispatch = useAppDispatch();
  const isInitialLoading = useAppSelector((state) => state.history.isLoading);
  const history = useAppSelector((state) => state.history.chats, {
    devModeChecks: { stabilityCheck: "never" },
  });

  const [searchQuery, setSearchQuery] = useState("");
  const [createTask] = useCreateTaskMutation();

  const {
    loadMore: loadMoreAsync,
    hasMore,
    isLoading: isLoadingMore,
    error: loadMoreError,
    retry: retryLoadMore,
  } = useLoadMoreHistory();

  const tree = useMemo(() => buildHistoryTree(history), [history]);

  const filteredTree = useMemo(() => {
    if (!searchQuery.trim()) return tree;
    const q = searchQuery.toLowerCase();
    return tree.filter((n) => treeMatchesQuery(n, q));
  }, [tree, searchQuery]);

  const handleItemClick = useCallback(
    (node: HistoryTreeNode) => {
      const item = history[node.id] as ChatHistoryItem | undefined;
      dispatch(restoreChat(item ?? (node as unknown as ChatHistoryItem)));
      dispatch(push({ name: "chat" }));
    },
    [dispatch, history],
  );

  const handleDotClick = useCallback(
    (chatId: string) => {
      const item = history[chatId] as ChatHistoryItem | undefined;
      if (item) {
        dispatch(restoreChat(item));
        dispatch(push({ name: "chat" }));
      }
    },
    [dispatch, history],
  );

  const handleDelete = useCallback(
    (id: string) => {
      dispatch(deleteChatById(id));
    },
    [dispatch],
  );

  const handleRename = useCallback(
    (id: string, newTitle: string) => {
      dispatch(updateChatTitleById({ chatId: id, newTitle }));
    },
    [dispatch],
  );

  const handleNewChat = useCallback(() => {
    dispatch(newChatAction());
    dispatch(push({ name: "chat" }));
  }, [dispatch]);

  const handleNewTask = useCallback(() => {
    void createTask({ name: "New Task" })
      .unwrap()
      .then((task) => {
        dispatch(push({ name: "task workspace", taskId: task.id }));
      });
  }, [createTask, dispatch]);

  const GROUP_ORDER = ["Today", "Yesterday", "Last 7 days", "Older"];

  // Build flat list for virtualization with group headers
  const flatItems = useMemo(() => {
    if (!expanded) return null;
    const groups = new Map<string, HistoryTreeNode[]>();
    for (const label of GROUP_ORDER) {
      groups.set(label, []);
    }
    for (const node of filteredTree) {
      const group = getDateGroup(node.updatedAt);
      if (!groups.has(group)) groups.set(group, []);
      const arr = groups.get(group);
      if (arr) arr.push(node);
    }
    const items: ({ type: "header"; label: string } | { type: "item"; node: HistoryTreeNode })[] = [];
    for (const [key, nodes] of groups) {
      if (nodes.length > 0) {
        items.push({ type: "header", label: key });
        for (const node of nodes) {
          items.push({ type: "item", node });
        }
      }
    }
    return items;
  }, [expanded, filteredTree]);

  const handleEndReached = useCallback(() => {
    if (hasMore && !isLoadingMore) {
      void loadMoreAsync();
    }
  }, [hasMore, isLoadingMore, loadMoreAsync]);

  return (
    <div className={styles.section}>
      <div className={styles.header}>
        <button
          type="button"
          className={styles.headerToggle}
          onClick={onToggleExpand}
        >
          <Text size="1" weight="bold" color="gray" className={styles.label}>
            RECENT
          </Text>
          <Flex align="center" gap="1">
            {!expanded && (
              <Text size="1" color="gray">
                {filteredTree.length} total
              </Text>
            )}
            {expanded ? (
              <ChevronUpIcon width={12} height={12} color="var(--gray-9)" />
            ) : (
              <ChevronDownIcon width={12} height={12} color="var(--gray-9)" />
            )}
          </Flex>
        </button>
        <Flex gap="1" align="center" className={styles.headerActions}>
          <Tooltip content="New Chat">
            <IconButton size="1" variant="ghost" color="gray" onClick={handleNewChat}>
              <PlusIcon width={14} height={14} />
            </IconButton>
          </Tooltip>
          <Tooltip content="New Task">
            <IconButton size="1" variant="ghost" color="gray" onClick={handleNewTask}>
              <CheckboxIcon width={14} height={14} />
            </IconButton>
          </Tooltip>
        </Flex>
      </div>

      {expanded && (
        <div className={styles.controls}>
          <TextField.Root
            size="1"
            placeholder="Search..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          >
            <TextField.Slot>
              <MagnifyingGlassIcon width={12} height={12} />
            </TextField.Slot>
          </TextField.Root>
        </div>
      )}

      <div className={styles.list}>
        {isInitialLoading && filteredTree.length === 0 ? (
          <Flex direction="column" gap="1" p="1">
            {Array.from({ length: 8 }, (_, i) => (
              <Flex key={i} align="center" gap="2" py="1" px="2">
                <Skeleton><div style={{ width: 8, height: 8, borderRadius: "50%" }} /></Skeleton>
                <Skeleton><Text size="2" style={{ width: `${120 + (i % 3) * 40}px` }}>&nbsp;</Text></Skeleton>
                <div style={{ flex: 1 }} />
                <Skeleton><Text size="1" style={{ width: 40 }}>&nbsp;</Text></Skeleton>
              </Flex>
            ))}
          </Flex>
        ) : expanded && flatItems ? (
          <Virtuoso
            data={flatItems}
            endReached={handleEndReached}
            overscan={200}
            className={styles.virtuosoList}
            itemContent={(_index, item) => {
              if (item.type === "header") {
                return (
                  <Text
                    size="1"
                    color="gray"
                    className={styles.groupLabel}
                  >
                    {item.label}
                  </Text>
                );
              }
              return (
                <RecentItem
                  node={item.node}
                  breakpoint={breakpoint}
                  onClick={() => handleItemClick(item.node)}
                  onDotClick={handleDotClick}
                  onDelete={handleDelete}
                  onRename={handleRename}
                />
              );
            }}
            components={{
              Footer: () => (
                <>
                  {isLoadingMore && (
                    <Flex justify="center" py="2">
                      <Spinner size="2" />
                    </Flex>
                  )}
                  {loadMoreError && (
                    <Flex justify="center" py="2">
                      <Text size="1" color="red" style={{ cursor: "pointer" }} onClick={retryLoadMore}>
                        Load failed — click to retry
                      </Text>
                    </Flex>
                  )}
                </>
              ),
            }}
          />
        ) : (
          <Virtuoso
            data={filteredTree}
            endReached={handleEndReached}
            overscan={200}
            className={styles.virtuosoList}
            itemContent={(_index, node) => (
              <RecentItem
                node={node}
                breakpoint={breakpoint}
                onClick={() => handleItemClick(node)}
                onDotClick={handleDotClick}
                onDelete={handleDelete}
                onRename={handleRename}
              />
            )}
            components={{
              Footer: () => (
                <>
                  {isLoadingMore && (
                    <Flex justify="center" py="2"><Spinner size="2" /></Flex>
                  )}
                </>
              ),
            }}
          />
        )}
        {!isInitialLoading && filteredTree.length === 0 && (
          <Text size="2" color="gray" style={{ padding: "var(--space-4)", textAlign: "center" }}>
            {searchQuery ? "No matching chats" : "No chats yet — start a new one!"}
          </Text>
        )}
      </div>
    </div>
  );
};
