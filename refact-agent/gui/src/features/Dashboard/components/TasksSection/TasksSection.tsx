import React, { useCallback } from "react";
import { Badge, Text } from "@radix-ui/themes";
import { useAppDispatch } from "../../../../hooks";
import { push } from "../../../Pages/pagesSlice";
import { useListTasksQuery } from "../../../../services/refact/tasks";
import { StatusDot } from "../../../../components/StatusDot";
import { getTaskStatusDotState } from "../../../../utils/sessionStatus";
import type { TaskMeta } from "../../../../services/refact/tasks";
import type { DashboardBreakpoint } from "../../types";
import styles from "./TasksSection.module.css";

type TasksSectionProps = {
  breakpoint: DashboardBreakpoint;
  compact?: boolean;
};

export const TasksSection: React.FC<TasksSectionProps> = ({
  breakpoint,
  compact,
}) => {
  const dispatch = useAppDispatch();
  const { data: tasks } = useListTasksQuery(undefined);

  const activeTasks = React.useMemo(() => {
    if (!tasks) return [];
    return tasks.filter(
      (t) => t.status === "active" || t.status === "planning" || t.status === "paused",
    );
  }, [tasks]);

  const handleTaskClick = useCallback(
    (task: TaskMeta) => {
      dispatch(push({ name: "task workspace", taskId: task.id }));
    },
    [dispatch],
  );

  if (activeTasks.length === 0) return null;

  if (compact) {
    return (
      <Text size="1" color="gray">
        📋 {activeTasks.length} active tasks
      </Text>
    );
  }

  return (
    <div className={styles.section}>
      <Text size="1" weight="bold" color="gray" className={styles.label}>
        📋 ACTIVE TASKS ({activeTasks.length})
      </Text>
      <div className={styles.list}>
        {activeTasks.slice(0, 4).map((task) => (
          <button
            key={task.id}
            type="button"
            className={styles.taskRow}
            onClick={() => handleTaskClick(task)}
          >
            <StatusDot state={getTaskStatusDotState(task)} size="small" />
            <Text size="1" truncate className={styles.taskName}>
              {task.name}
            </Text>
            {breakpoint !== "narrow" && (
              <Text size="1" color="gray">
                {task.cards_done}/{task.cards_total} cards
              </Text>
            )}
            {breakpoint !== "narrow" && task.agents_active > 0 && (
              <Text size="1" color="gray">
                {task.agents_active} agents
              </Text>
            )}
            <Badge size="1" variant="soft" color={
              task.status === "active" ? "blue" :
              task.status === "planning" ? "purple" : "amber"
            }>
              {task.status}
            </Badge>
          </button>
        ))}
      </div>
    </div>
  );
};
