import type { HistoryTreeNode } from "../../../History/historySlice";

export type TrailDot = {
  id: string;
  chatId: string;
  type: "user" | "assistant" | "fork" | "active" | "completed";
  label?: string;
  depth: number;
  hasBranch: boolean;
};

export function buildDotTrail(
  node: HistoryTreeNode,
  maxDots: number = 8,
): TrailDot[] {
  const dots: TrailDot[] = [];

  function addDot(
    n: HistoryTreeNode,
    depth: number,
  ) {
    if (dots.length >= maxDots) return;

    const isActive =
      n.session_state === "generating" ||
      n.session_state === "executing_tools";
    const isCompleted = n.session_state === "completed";
    const hasBranch = n.children.length > 1;

    let dotType: TrailDot["type"];
    if (isActive) dotType = "active";
    else if (isCompleted) dotType = "completed";
    else if (hasBranch) dotType = "fork";
    else if (n.link_type === "handoff" || n.link_type === "subagent")
      dotType = "assistant";
    else dotType = "user";

    dots.push({
      id: n.id,
      chatId: n.id,
      type: dotType,
      label: n.link_type ?? undefined,
      depth,
      hasBranch,
    });

    for (const child of n.children) {
      if (dots.length >= maxDots) break;
      addDot(child, depth + 1);
    }
  }

  addDot(node, 0);
  return dots;
}
