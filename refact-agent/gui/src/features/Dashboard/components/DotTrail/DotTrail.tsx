import React, { useMemo } from "react";
import { Flex, HoverCard, Text } from "@radix-ui/themes";
import type { HistoryTreeNode } from "../../../History/historySlice";
import type { DashboardBreakpoint } from "../../types";
import { buildDotTrail, type TrailDot } from "./buildDotTrail";
import styles from "./DotTrail.module.css";

type DotTrailProps = {
  node: HistoryTreeNode;
  breakpoint: DashboardBreakpoint;
  onDotClick?: (chatId: string) => void;
};

const DOT_SIZE: Record<DashboardBreakpoint, number> = {
  narrow: 10,
  medium: 11,
  wide: 12,
};

const GAP: Record<DashboardBreakpoint, number> = {
  narrow: 4,
  medium: 5,
  wide: 6,
};

function DotWithTooltip({ dot, cx, cy, r, className, style, onClick, node }: {
  dot: TrailDot;
  cx: number;
  cy: number;
  r: number;
  className: string;
  style: React.CSSProperties;
  onClick?: React.MouseEventHandler;
  node: HistoryTreeNode;
}) {
  const circle = (
    <circle
      cx={cx}
      cy={cy}
      r={r}
      className={className}
      onClick={onClick}
      style={style}
    />
  );

  return (
    <HoverCard.Root openDelay={300} closeDelay={100}>
      <HoverCard.Trigger>
        <g>{circle}</g>
      </HoverCard.Trigger>
      <HoverCard.Content size="1" side="top" align="center" avoidCollisions>
        <Flex direction="column" gap="1" style={{ maxWidth: 220 }}>
          <Text size="1" weight="bold" truncate>{dot.label ?? dot.type}</Text>
          {node.model && <Text size="1" color="gray">Model: {node.model}</Text>}
          {node.mode && <Text size="1" color="gray">Mode: {node.mode}</Text>}
          {(node.message_count ?? 0) > 0 && <Text size="1" color="gray">Messages: {node.message_count}</Text>}
          {node.session_state && node.session_state !== "idle" && (
            <Text size="1" color="gray">Status: {node.session_state}</Text>
          )}
        </Flex>
      </HoverCard.Content>
    </HoverCard.Root>
  );
}

function findNodeById(node: HistoryTreeNode, id: string): HistoryTreeNode | undefined {
  if (node.id === id) return node;
  for (const child of node.children) {
    const found = findNodeById(child, id);
    if (found) return found;
  }
  return undefined;
}

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
  const forkDotSize = dotSize + 3;
  const totalWidth = dots.length * (dotSize + gap) - gap;
  const height = dotSize + 6;

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
        const dotNode = findNodeById(node, dot.chatId) ?? node;

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
            <DotWithTooltip
              dot={dot}
              cx={x}
              cy={y}
              r={r}
              className={`${styles.dot} ${styles[dot.type]}`}
              style={{ cursor: onDotClick ? "pointer" : "default" }}
              onClick={onDotClick ? (e: React.MouseEvent) => {
                e.stopPropagation();
                onDotClick(dot.chatId);
              } : undefined}
              node={dotNode}
            />
          </g>
        );
      })}
    </svg>
  );
};
