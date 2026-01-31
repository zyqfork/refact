import React, { useCallback } from "react";
import * as Collapsible from "@radix-ui/react-collapsible";
import { Flex, Text, Box, Separator } from "@radix-ui/themes";
import { Spinner } from "@radix-ui/themes";
import classNames from "classnames";

import { useAppSelector, useAppDispatch } from "../../hooks";
import {
  selectChatId,
  selectCurrentTasks,
  selectHasTasks,
  selectTasksEverUsed,
  selectTaskProgress,
  selectTaskWidgetExpanded,
  selectIsStreaming,
  setTaskWidgetExpanded,
} from "../../features/Chat/Thread";
import type { TodoItem, TodoStatus } from "../../features/Chat/Thread/types";
import { Chevron } from "../Collapsible";
import { AnimatedText } from "../Text";
import styles from "./TaskProgressWidget.module.css";

const STATUS_ICONS: Record<TodoStatus, string> = {
  completed: "✅",
  in_progress: "🔄",
  pending: "⏳",
  failed: "❌",
};

type StatusIconProps = {
  status: TodoStatus;
  showSpinner?: boolean;
};

const StatusIcon: React.FC<StatusIconProps> = ({ status, showSpinner }) => {
  if (status === "in_progress" && showSpinner) {
    return <Spinner size="1" />;
  }
  return <span>{STATUS_ICONS[status]}</span>;
};

type TaskRowProps = {
  task: TodoItem;
};

const TaskRow: React.FC<TaskRowProps> = ({ task }) => {
  const isActive = task.status === "in_progress";

  return (
    <Flex
      align="center"
      gap="2"
      className={classNames(styles.taskRow, { [styles.active]: isActive })}
    >
      <StatusIcon status={task.status} showSpinner={false} />
      <Text size="2" style={{ flex: 1 }}>
        {task.content}
      </Text>
      {isActive && (
        <Text size="1" color="blue">
          ● active
        </Text>
      )}
    </Flex>
  );
};

type ProgressBarProps = {
  done: number;
  total: number;
};

const ProgressBar: React.FC<ProgressBarProps> = ({ done, total }) => {
  const percent = total > 0 ? (done / total) * 100 : 0;

  return (
    <Box className={styles.progressBar}>
      <Box
        className={styles.progressFill}
        style={{ width: `${percent}%` }}
      />
    </Box>
  );
};

export const TaskProgressWidget: React.FC = () => {
  const dispatch = useAppDispatch();
  const chatId = useAppSelector(selectChatId);
  const hasTasks = useAppSelector(selectHasTasks);
  const everUsed = useAppSelector(selectTasksEverUsed);
  const tasks = useAppSelector(selectCurrentTasks);
  const isExpanded = useAppSelector(selectTaskWidgetExpanded);
  const isStreaming = useAppSelector(selectIsStreaming);
  const { done, total, activeTitle } = useAppSelector(selectTaskProgress);

  const handleOpenChange = useCallback(
    (open: boolean) => {
      if (chatId) {
        dispatch(setTaskWidgetExpanded({ id: chatId, expanded: open }));
      }
    },
    [dispatch, chatId],
  );

  if (!everUsed) return null;

  const hasActive = tasks.some((t) => t.status === "in_progress");
  const isAnimating = hasActive && isStreaming;

  return (
    <Box className={styles.widget}>
      <Collapsible.Root open={isExpanded} onOpenChange={handleOpenChange}>
        <Collapsible.Trigger asChild>
          <Flex
            className={styles.header}
            align="center"
            gap="3"
            px="3"
            py="2"
          >
            <AnimatedText as="div" size="1" animating={isAnimating}>
              <Flex align="center" gap="2" style={{ flex: 1 }}>
                <Text>📋</Text>

                {!isExpanded && hasTasks && (
                  <>
                    <Flex gap="1">
                      {tasks.map((task) => (
                        <StatusIcon
                          key={task.id}
                          status={task.status}
                          showSpinner={task.status === "in_progress" && isStreaming}
                        />
                      ))}
                    </Flex>

                    <Text size="1" color="gray">
                      {done}/{total}
                    </Text>

                    <ProgressBar done={done} total={total} />

                    {activeTitle && (
                      <Text size="1" color="gray" className={styles.activeHint}>
                        {activeTitle}
                      </Text>
                    )}
                  </>
                )}

                {!isExpanded && !hasTasks && (
                  <Text size="1" color="gray">
                    Tasks cleared
                  </Text>
                )}

                {isExpanded && (
                  <Text size="1" weight="medium">
                    Task Progress
                  </Text>
                )}
              </Flex>
            </AnimatedText>

            <Chevron open={isExpanded} />
          </Flex>
        </Collapsible.Trigger>

        <Collapsible.Content>
          <Flex direction="column" gap="2" px="3" pb="3">
            {hasTasks ? (
              <>
                {tasks.map((task) => (
                  <TaskRow key={task.id} task={task} />
                ))}
                <Separator size="4" />
                <Text size="1" color="gray">
                  {done}/{total} completed
                </Text>
              </>
            ) : (
              <Text size="1" color="gray">
                No active tasks
              </Text>
            )}
          </Flex>
        </Collapsible.Content>
      </Collapsible.Root>
    </Box>
  );
};

export default TaskProgressWidget;
