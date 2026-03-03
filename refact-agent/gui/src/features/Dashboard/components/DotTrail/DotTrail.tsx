import React, { useMemo } from "react";
import type { HistoryTreeNode } from "../../../History/historySlice";
import type { DashboardBreakpoint } from "../../types";
import { buildDotTrail } from "./buildDotTrail";
import styles from "./DotTrail.module.css";

type DotTrailProps = {
  node: HistoryTreeNode;
  breakpoint: DashboardBreakpoint;
  onDotClick?: (chatId: string) => void;
};

const DOT_SIZE: Record<DashboardBreakpoint, number> = {
  narrow: 6,
  medium: 7,
  wide: 8,
};

const GAP: Record<DashboardBreakpoint, number> = {
  narrow: 3,
  medium: 4,
  wide: 5,
};

export const DotTrail: React.FC<DotTrailProps> = ({
  node,
  breakpoint,
  onDotClick,
}) => {
  const maxDots = breakpoint === "narrow" ? 6 : breakpoint === "medium" ? 8 : 10;
  const dots = useMemo(() => buildDotTrail(node, maxDots), [node, maxDots]);

  if (dots.length <= 1) return null;

  const dotSize = DOT_SIZE[breakpoint];
  const gap = GAP[breakpoint];
  const forkDotSize = dotSize + 2;
  const totalWidth = dots.length * (dotSize + gap) - gap;
  const height = dotSize + 4;

  return (
    <svg
      width={totalWidth}
      height={height}
      className={styles.trail}
      aria-label="Thread trail"
    >
      {dots.map((dot, i) => {
        const x = i * (dotSize + gap) + dotSize / 2;
        const y = height / 2;
        const r = dot.hasBranch ? forkDotSize / 2 : dotSize / 2;

        return (
          <g key={dot.id}>
            {i > 0 && breakpoint !== "narrow" && (
              <line
                x1={(i - 1) * (dotSize + gap) + dotSize / 2}
                y1={y}
                x2={x}
                y2={y}
                stroke="var(--gray-6)"
                strokeWidth={1}
              />
            )}
            <circle
              cx={x}
              cy={y}
              r={r}
              className={`${styles.dot} ${styles[dot.type]}`}
              onClick={onDotClick ? (e: React.MouseEvent) => {
                e.stopPropagation();
                onDotClick(dot.chatId);
              } : undefined}
              style={{ cursor: onDotClick ? "pointer" : "default" }}
            >
              <title>
                {dot.label
                  ? `${dot.type} (${dot.label})`
                  : dot.type}
              </title>
            </circle>
          </g>
        );
      })}
    </svg>
  );
};
