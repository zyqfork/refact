import { describe, expect, it } from "vitest";
import { render, screen } from "../../utils/test-utils";
import { TaskDocumentsContent } from "./TaskDocumentsView";

const DOC_LIST = [
  "| slug | name | kind | pinned | version | updated_at |",
  "|---|---|---|---|---:|---|",
  "| main-plan | Main Plan | plan | true | 3 | 2026-05-22T10:00:00Z |",
  "| api-spec | API Spec | spec | false | 1 | 2026-05-22T11:00:00Z |",
  "| runbook | Deploy Runbook | runbook | true | 2 | 2026-05-22T12:00:00Z |",
].join("\n");

const DOC_GET = [
  "---",
  "name: Main Plan",
  "slug: main-plan",
  "kind: plan",
  "pinned: true",
  "version: 3",
  "---",
  "",
  "# Main Plan",
  "",
  "- Ship document renderer",
].join("\n");

describe("TaskDocumentsContent", () => {
  it("parses doc_list markdown into rows and renders them", () => {
    render(<TaskDocumentsContent toolType="doc_list" content={DOC_LIST} />);

    expect(screen.getByText("Main Plan")).toBeInTheDocument();
    expect(screen.getByText("API Spec")).toBeInTheDocument();
    expect(screen.getByText("Deploy Runbook")).toBeInTheDocument();
    expect(screen.getByText("3 documents")).toBeInTheDocument();
  });

  it("renders doc_get body markdown", () => {
    render(<TaskDocumentsContent toolType="doc_get" content={DOC_GET} />);

    expect(
      screen.getByRole("heading", { name: "Main Plan" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Ship document renderer")).toBeInTheDocument();
    expect(screen.getByText("main-plan")).toBeInTheDocument();
    expect(screen.getByText("v3")).toBeInTheDocument();
  });

  it("renders pinned and non-pinned stars differently", () => {
    render(<TaskDocumentsContent toolType="doc_list" content={DOC_LIST} />);

    expect(screen.getByLabelText("Pinned main-plan")).toHaveTextContent("★");
    expect(screen.getByLabelText("Not pinned api-spec")).toHaveTextContent("☆");
  });
});
