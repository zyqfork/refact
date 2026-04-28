import { render, screen, waitFor } from "../utils/test-utils";
import { http, HttpResponse } from "msw";
import { describe, expect, it, vi } from "vitest";
import { server } from "../utils/mockServer";
import { SkillEditor } from "../features/Extensions/components/SkillEditor";
import { CommandEditor } from "../features/Extensions/components/CommandEditor";
import { BuddyDraftPreview } from "../features/Buddy/BuddyDraftPreview";
import { ConfigEditor } from "../features/Customization/Customization";
import type { BuddyDraft } from "../features/Buddy/types";
import type { ConfigItem } from "../services/refact/customization";

const CONFIG_STATE = {
  config: {
    apiKey: "test",
    lspPort: 8001,
    themeProps: {},
    host: "vscode" as const,
  },
};

const SKILL_DRAFT: BuddyDraft = {
  id: "draft-skill-1",
  kind: "skill",
  title: "My Skill Draft",
  yaml_or_json: "---\ndescription: Draft description\n---\n# Content",
  explanation: "Buddy suggests adding this skill",
  created_at: "2024-01-01T00:00:00Z",
  expires_at: "2099-12-31T00:00:00Z",
};

const COMMAND_DRAFT: BuddyDraft = {
  id: "draft-cmd-1",
  kind: "command",
  title: "My Command Draft",
  yaml_or_json: "---\ndescription: Command desc\n---\n# Command body",
  explanation: "Buddy suggests this command",
  created_at: "2024-01-01T00:00:00Z",
  expires_at: "2099-12-31T00:00:00Z",
};

const MOCK_SKILL_DETAIL = {
  name: "my_skill",
  description: "Existing description",
  user_invocable: true,
  disable_model_invocation: false,
  allowed_tools: [],
  model: null,
  context: null,
  agent: null,
  argument_hint: "",
  body: "# Body",
  raw_content: "---\ndescription: Existing description\n---\n# Body",
  source: "global",
  file_path: "/home/.config/refact/skills/my_skill/SKILL.md",
};

const MOCK_COMMAND_DETAIL = {
  name: "my_cmd",
  description: "Existing cmd desc",
  argument_hint: "",
  allowed_tools: [],
  model: null,
  body: "# Cmd body",
  raw_content: "---\ndescription: Existing cmd desc\n---\n# Cmd body",
  source: "global",
  file_path: "/home/.config/refact/commands/my_cmd.md",
};

const MOCK_CONFIG_ITEM: ConfigItem = {
  id: "my_mode",
  kind: "modes",
  title: "My Mode",
  file_path: "/global/modes/my_mode.yaml",
  specific: false,
  scope: "global",
  global_path: "/global/modes/my_mode.yaml",
  local_path: "",
  global_exists: true,
  local_exists: false,
};

const MOCK_CONFIG_DETAIL = {
  config: { id: "my_mode", title: "My Mode", prompt: "existing prompt" },
  file_path: "/global/modes/my_mode.yaml",
  raw_yaml: "id: my_mode\ntitle: My Mode\nprompt: existing prompt\n",
  scope: "global" as const,
};

const MODE_DRAFT: BuddyDraft = {
  id: "draft-mode-1",
  kind: "mode",
  title: "My Mode Draft",
  yaml_or_json: "id: my_mode\ntitle: My Mode\nprompt: draft prompt\n",
  explanation: "Buddy suggests this mode change",
  created_at: "2024-01-01T00:00:00Z",
  expires_at: "2099-12-31T00:00:00Z",
};

describe("BuddyDraftPreview_visible_when_draft_present", () => {
  it("renders draft title and explanation", () => {
    render(<BuddyDraftPreview draft={SKILL_DRAFT} />, {
      preloadedState: CONFIG_STATE,
    });

    expect(screen.getByText("Buddy Draft: My Skill Draft")).toBeDefined();
    expect(screen.getByText("Buddy suggests adding this skill")).toBeDefined();
  });

  it("renders without explanation when not provided", () => {
    const draft: BuddyDraft = { ...SKILL_DRAFT, explanation: "" };
    render(<BuddyDraftPreview draft={draft} />, {
      preloadedState: CONFIG_STATE,
    });
    expect(screen.getByText("Buddy Draft: My Skill Draft")).toBeDefined();
  });
});

describe("SkillEditor_kind_mismatch_shows_error", () => {
  it("shows kind mismatch when command draft given to skill editor", async () => {
    server.use(
      http.get("http://127.0.0.1:8001/v1/ext/skills/my_skill", () =>
        HttpResponse.json(MOCK_SKILL_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-cmd-1", () =>
        HttpResponse.json(COMMAND_DRAFT),
      ),
    );

    render(
      <SkillEditor name="my_skill" onBack={vi.fn()} draftId="draft-cmd-1" />,
      { preloadedState: CONFIG_STATE },
    );

    const mismatch = await screen.findByText(
      "Draft kind mismatch: expected skill draft",
    );
    expect(mismatch).toBeDefined();
  });
});

describe("SkillEditor with correct draft_id", () => {
  it("shows BuddyDraftPreview when skill draft present", async () => {
    server.use(
      http.get("http://127.0.0.1:8001/v1/ext/skills/my_skill", () =>
        HttpResponse.json(MOCK_SKILL_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-skill-1", () =>
        HttpResponse.json(SKILL_DRAFT),
      ),
    );

    render(
      <SkillEditor name="my_skill" onBack={vi.fn()} draftId="draft-skill-1" />,
      { preloadedState: CONFIG_STATE },
    );

    const preview = await screen.findByText("Buddy Draft: My Skill Draft");
    expect(preview).toBeDefined();
    expect(screen.getByText("Buddy suggests adding this skill")).toBeDefined();
  });

  it("prefills raw content from draft", async () => {
    server.use(
      http.get("http://127.0.0.1:8001/v1/ext/skills/my_skill", () =>
        HttpResponse.json(MOCK_SKILL_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-skill-1", () =>
        HttpResponse.json(SKILL_DRAFT),
      ),
    );

    render(
      <SkillEditor name="my_skill" onBack={vi.fn()} draftId="draft-skill-1" />,
      { preloadedState: CONFIG_STATE },
    );

    await screen.findByText("Buddy Draft: My Skill Draft");

    const textareas = document.querySelectorAll("textarea");
    const rawTextarea = Array.from(textareas).find((t) =>
      t.value.includes("Draft description"),
    );
    expect(rawTextarea).toBeDefined();
    expect(rawTextarea?.value).toContain("Draft description");
  });

  it("passes draft_id to save mutation", async () => {
    let savedBody: Record<string, unknown> | null = null;

    server.use(
      http.get("http://127.0.0.1:8001/v1/ext/skills/my_skill", () =>
        HttpResponse.json(MOCK_SKILL_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-skill-1", () =>
        HttpResponse.json(SKILL_DRAFT),
      ),
      http.put(
        "http://127.0.0.1:8001/v1/ext/skills/my_skill",
        async ({ request }) => {
          savedBody = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({});
        },
      ),
    );

    render(
      <SkillEditor name="my_skill" onBack={vi.fn()} draftId="draft-skill-1" />,
      { preloadedState: CONFIG_STATE },
    );

    await screen.findByText("Buddy Draft: My Skill Draft");

    const saveBtn = screen.getByText("Save");
    saveBtn.click();

    await waitFor(() => {
      expect(savedBody).not.toBeNull();
      expect(savedBody?.draft_id).toBe("draft-skill-1");
    });
  });
});

describe("CommandEditor with correct draft_id", () => {
  it("shows BuddyDraftPreview when command draft present", async () => {
    server.use(
      http.get("http://127.0.0.1:8001/v1/ext/commands/my_cmd", () =>
        HttpResponse.json(MOCK_COMMAND_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-cmd-1", () =>
        HttpResponse.json(COMMAND_DRAFT),
      ),
    );

    render(
      <CommandEditor name="my_cmd" onBack={vi.fn()} draftId="draft-cmd-1" />,
      { preloadedState: CONFIG_STATE },
    );

    const preview = await screen.findByText("Buddy Draft: My Command Draft");
    expect(preview).toBeDefined();
  });
});

describe("CustomizationEditor_with_draft_id_prefills", () => {
  it("shows BuddyDraftPreview and uses draft yaml when mode draft present", async () => {
    server.use(
      http.get("http://127.0.0.1:8001/v1/customization/modes/my_mode", () =>
        HttpResponse.json(MOCK_CONFIG_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-mode-1", () =>
        HttpResponse.json(MODE_DRAFT),
      ),
    );

    render(
      <ConfigEditor
        kind="modes"
        configId="my_mode"
        configItem={MOCK_CONFIG_ITEM}
        onSaved={vi.fn()}
        draftId="draft-mode-1"
      />,
      { preloadedState: CONFIG_STATE },
    );

    const preview = await screen.findByText("Buddy Draft: My Mode Draft");
    expect(preview).toBeDefined();
    expect(screen.getByText("Buddy suggests this mode change")).toBeDefined();
  });

  it("CustomizationEditor_save_with_draft_id_passes_through", async () => {
    let savedBody: Record<string, unknown> | null = null;

    server.use(
      http.get("http://127.0.0.1:8001/v1/customization/modes/my_mode", () =>
        HttpResponse.json(MOCK_CONFIG_DETAIL),
      ),
      http.get("http://127.0.0.1:8001/v1/buddy/drafts/draft-mode-1", () =>
        HttpResponse.json(MODE_DRAFT),
      ),
      http.put(
        "http://127.0.0.1:8001/v1/customization/modes/my_mode",
        async ({ request }) => {
          savedBody = (await request.json()) as Record<string, unknown>;
          return HttpResponse.json({
            ok: true,
            file_path: "/global/modes/my_mode.yaml",
            scope: "global",
            errors: [],
          });
        },
      ),
    );

    render(
      <ConfigEditor
        kind="modes"
        configId="my_mode"
        configItem={MOCK_CONFIG_ITEM}
        onSaved={vi.fn()}
        draftId="draft-mode-1"
      />,
      { preloadedState: CONFIG_STATE },
    );

    await screen.findByText("Buddy Draft: My Mode Draft");

    const saveBtn = screen.getByText("Save");
    saveBtn.click();

    await waitFor(() => {
      expect(savedBody).not.toBeNull();
      expect(savedBody?.draft_id).toBe("draft-mode-1");
    });
  });
});
