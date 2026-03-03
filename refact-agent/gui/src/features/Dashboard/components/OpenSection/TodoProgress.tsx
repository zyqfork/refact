import React from "react";
import { Text } from "@radix-ui/themes";
import type { TodoItem } from "../../../Chat/Thread/types";
import type { DashboardBreakpoint } from "../../types";
import styles from "./TodoProgress.module.css";

type TodoProgressProps = {
  todos: TodoItem[];
  breakpoint: DashboardBreakpoint;
};

export const TodoProgress: React.FC<TodoProgressProps> = ({
  todos,
  breakpoint,
}) => {
  if (todos.length === 0) return null;

  const done = todos.filter((t) => t.status === "completed").length;
  const total = todos.length;

  if (breakpoint === "narrow") {
    return (
      <div className={styles.compact}>
        <Text size="1" color="gray">
          ☑{done}/{total}
        </Text>
        <div className={styles.miniBar}>
          {todos.slice(0, 12).map((t) => (
            <div
              key={t.id}
              className={styles.miniSegment}
              data-status={t.status}
            />
          ))}
        </div>
      </div>
    );
  }

  const MAX_VISIBLE = 3;
  const visible = todos.slice(0, MAX_VISIBLE);
  const remaining = todos.length - MAX_VISIBLE;

  return (
    <div className={styles.list}>
      {visible.map((t) => (
        <div key={t.id} className={styles.item}>
          <span className={styles.statusDot} data-status={t.status} />
          <Text size="1" truncate className={styles.itemText}>
            {t.content}
          </Text>
        </div>
      ))}
      {remaining > 0 && (
        <Text size="1" color="gray">+{remaining} more</Text>
      )}
    </div>
  );
};
