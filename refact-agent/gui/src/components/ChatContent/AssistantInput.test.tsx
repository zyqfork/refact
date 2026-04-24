import { describe, expect, test } from "vitest";
import { render, screen } from "../../utils/test-utils";
import { AssistantInput } from "./AssistantInput";
import type { DiffChunk, ToolCall } from "../../services/refact/types";

describe("AssistantInput", () => {
  test("renders streaming message content as markdown immediately", () => {
    const { rerender } = render(
      <AssistantInput message="## Streaming title" isStreaming />,
    );

    expect(
      screen.getByRole("heading", { name: "Streaming title" }),
    ).toBeInTheDocument();

    rerender(<AssistantInput message="## Streaming title" />);

    expect(
      screen.getByRole("heading", { name: "Streaming title" }),
    ).toBeInTheDocument();
  });

  test("passes diffsByToolId through to serverExecutedTools so diff blocks are not repeated outside the tool card", () => {
    const toolCall: ToolCall = {
      id: "call-1",
      index: 0,
      function: {
        name: "apply_patch",
        arguments: "{}",
      },
    };

    const diffChunk: DiffChunk = {
      file_name: "debug_codex_models.py",
      file_action: "edit",
      line1: 10,
      line2: 11,
      lines_remove: "old line",
      lines_add: "new line",
    };

    render(
      <AssistantInput
        message="I'll update the debug script."
        serverExecutedTools={[toolCall]}
        diffsByToolId={{ "call-1": [diffChunk] }}
      />,
    );

    expect(screen.getAllByText(/debug_codex_models\.py/i)).toHaveLength(1);
    expect(screen.queryByText(/Tasks 0\/3/i)).not.toBeInTheDocument();
  });
});
