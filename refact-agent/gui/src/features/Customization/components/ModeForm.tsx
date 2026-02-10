import React, { useState, useCallback, useMemo } from "react";
import {
  Flex,
  TextField,
  Text,
  Switch,
  TextArea,
  Tabs,
  Select,
} from "@radix-ui/themes";
import { StringListEditor } from "./StringListEditor";
import { RulesTableEditor } from "./RulesTableEditor";
import {
  ConfigPatch,
  safeArray,
  safeString,
  safeBoolean,
  safeObject,
  isString,
  safeToolConfirmRules,
} from "./configUtils";
import { useGetCapsQuery } from "../../../services/refact/caps";
import { useCapsForToolUse } from "../../../hooks";
import { enrichAndGroupModels } from "../../../utils/enrichModels";
import { RichModelSelectItem } from "../../../components/Select/RichModelSelectItem";
import styles from "./editors.module.css";
import selectStyles from "../../../components/Select/select.module.css";

type ModeFormProps = {
  config: Record<string, unknown>;
  onPatch: (patch: ConfigPatch) => void;
  availableTools?: string[];
};

type ModelTypeSectionProps = {
  title: string;
  typeKey: "default" | "light" | "thinking";
  config: Record<string, unknown>;
  groupedModels: ReturnType<typeof enrichAndGroupModels>;
  onPatch: (path: (string | number)[], value: unknown) => void;
};

const ModelTypeSection: React.FC<ModelTypeSectionProps> = ({
  title,
  typeKey,
  config,
  groupedModels,
  onPatch,
}) => {
  const model = safeString(config.model);
  const maxNewTokens =
    typeof config.max_new_tokens === "number"
      ? config.max_new_tokens
      : undefined;
  const temperature =
    typeof config.temperature === "number" ? config.temperature : undefined;
  const topP = typeof config.top_p === "number" ? config.top_p : undefined;
  const boostReasoning =
    typeof config.boost_reasoning === "boolean"
      ? config.boost_reasoning
      : false;
  const reasoningEffort =
    typeof config.reasoning_effort === "string" ? config.reasoning_effort : "";
  const thinkingBudget =
    typeof config.thinking_budget === "number"
      ? config.thinking_budget
      : undefined;
  const toolChoice =
    typeof config.tool_choice === "string" ? config.tool_choice : "";
  const parallelToolCalls =
    typeof config.parallel_tool_calls === "boolean"
      ? config.parallel_tool_calls
      : false;

  const basePath = ["model_defaults", typeKey];

  return (
    <Flex
      direction="column"
      gap="2"
      p="2"
      style={{
        border: "1px solid var(--gray-6)",
        borderRadius: "var(--radius-2)",
      }}
    >
      <Text size="1" weight="medium">
        {title}
      </Text>
      <Flex direction="column" gap="1">
        <Text size="1" color="gray">
          Model
        </Text>
        <Select.Root
          value={model || "__inherit__"}
          onValueChange={(v) =>
            onPatch([...basePath, "model"], v === "__inherit__" ? undefined : v)
          }
          size="1"
        >
          <Select.Trigger
            placeholder="Inherit from global"
            style={{ width: "100%" }}
          />
          <Select.Content position="popper">
            <Select.Item value="__inherit__">
              <Text color="gray">Inherit from global</Text>
            </Select.Item>
            <Select.Separator />
            {groupedModels.map((group) => (
              <Select.Group key={group.provider}>
                <Select.Label>{group.displayName}</Select.Label>
                {group.models.map((m) => (
                  <Select.Item
                    key={m.value}
                    value={m.value}
                    textValue={m.value}
                  >
                    <span className={selectStyles.trigger_only}>{m.value}</span>
                    <span className={selectStyles.dropdown_only}>
                      <RichModelSelectItem
                        displayName={m.value}
                        pricing={m.pricing}
                        nCtx={m.nCtx}
                        capabilities={m.capabilities}
                        isDefault={m.isDefault}
                        isThinking={m.isThinking}
                        isLight={m.isLight}
                      />
                    </span>
                  </Select.Item>
                ))}
              </Select.Group>
            ))}
          </Select.Content>
        </Select.Root>
      </Flex>
      <Flex gap="2" wrap="wrap">
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 70 }}>
          <Text size="1" color="gray">
            Max Tokens
          </Text>
          <TextField.Root
            size="1"
            type="number"
            value={maxNewTokens?.toString() ?? ""}
            placeholder="Default"
            onChange={(e) =>
              onPatch(
                [...basePath, "max_new_tokens"],
                e.target.value ? parseInt(e.target.value, 10) : undefined,
              )
            }
          />
        </Flex>
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 70 }}>
          <Text size="1" color="gray">
            Temp
          </Text>
          <TextField.Root
            size="1"
            type="number"
            step="0.1"
            value={temperature?.toString() ?? ""}
            placeholder="Default"
            onChange={(e) =>
              onPatch(
                [...basePath, "temperature"],
                e.target.value ? parseFloat(e.target.value) : undefined,
              )
            }
          />
        </Flex>
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 70 }}>
          <Text size="1" color="gray">
            Top P
          </Text>
          <TextField.Root
            size="1"
            type="number"
            step="0.1"
            value={topP?.toString() ?? ""}
            placeholder="Default"
            onChange={(e) =>
              onPatch(
                [...basePath, "top_p"],
                e.target.value ? parseFloat(e.target.value) : undefined,
              )
            }
          />
        </Flex>
      </Flex>
      <Flex gap="2" wrap="wrap" align="center">
        <Flex align="center" gap="1">
          <Switch
            size="1"
            checked={boostReasoning}
            onCheckedChange={(c) =>
              onPatch([...basePath, "boost_reasoning"], c || undefined)
            }
          />
          <Text size="1">Boost</Text>
        </Flex>
        <Flex align="center" gap="1">
          <Switch
            size="1"
            checked={parallelToolCalls}
            onCheckedChange={(c) =>
              onPatch([...basePath, "parallel_tool_calls"], c || undefined)
            }
          />
          <Text size="1">Parallel</Text>
        </Flex>
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 80 }}>
          <Text size="1" color="gray">
            Effort
          </Text>
          <TextField.Root
            size="1"
            value={reasoningEffort}
            placeholder="low/medium/high/xhigh/max"
            onChange={(e) =>
              onPatch(
                [...basePath, "reasoning_effort"],
                e.target.value || undefined,
              )
            }
          />
        </Flex>
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 80 }}>
          <Text size="1" color="gray">
            Tool Choice
          </Text>
          <TextField.Root
            size="1"
            value={toolChoice}
            placeholder="auto/none"
            onChange={(e) =>
              onPatch([...basePath, "tool_choice"], e.target.value || undefined)
            }
          />
        </Flex>
        <Flex direction="column" gap="1" style={{ flex: 1, minWidth: 80 }}>
          <Text size="1" color="gray">
            Think Budget
          </Text>
          <TextField.Root
            size="1"
            type="number"
            value={thinkingBudget?.toString() ?? ""}
            placeholder="Default"
            onChange={(e) =>
              onPatch(
                [...basePath, "thinking_budget"],
                e.target.value ? parseInt(e.target.value, 10) : undefined,
              )
            }
          />
        </Flex>
      </Flex>
    </Flex>
  );
};

export const ModeForm: React.FC<ModeFormProps> = ({
  config,
  onPatch,
  availableTools = [],
}) => {
  const [activeTab, setActiveTab] = useState("basic");

  const title = safeString(config.title);
  const description = safeString(config.description);
  const specific = safeBoolean(config.specific);
  const prompt = safeString(config.prompt);
  const tools = safeArray(config.tools, isString);
  const modelDefaults = safeObject(config.model_defaults);
  const modelDefaultsDefault = safeObject(modelDefaults.default);
  const modelDefaultsLight = safeObject(modelDefaults.light);
  const modelDefaultsThinking = safeObject(modelDefaults.thinking);
  const toolConfirmObj = safeObject(config.tool_confirm);
  const toolConfirmRules = safeToolConfirmRules(toolConfirmObj.rules);
  const threadDefaults = safeObject(config.thread_defaults);
  const ui = safeObject(config.ui);
  const base = typeof config.base === "string" ? config.base : undefined;
  const matchModels = Array.isArray(config.match_models)
    ? safeArray(config.match_models, isString)
    : undefined;

  const patch = useCallback(
    (path: (string | number)[], value: unknown) => {
      onPatch({ path, value });
    },
    [onPatch],
  );

  const { data: capsData } = useGetCapsQuery(undefined);
  const capsForToolUse = useCapsForToolUse();

  // Use the same filtered model list as the main chat selector
  const groupedModels = useMemo(() => {
    return enrichAndGroupModels(capsForToolUse.usableModelsForPlan, capsData);
  }, [capsForToolUse.usableModelsForPlan, capsData]);

  return (
    <Tabs.Root
      value={activeTab}
      onValueChange={setActiveTab}
      style={{
        display: "flex",
        flexDirection: "column",
        flex: 1,
        minHeight: 0,
      }}
    >
      <Tabs.List>
        <Tabs.Trigger value="basic">Basic</Tabs.Trigger>
        <Tabs.Trigger value="tools">Tools</Tabs.Trigger>
        <Tabs.Trigger value="llm">LLM Settings</Tabs.Trigger>
        <Tabs.Trigger value="advanced">Advanced</Tabs.Trigger>
      </Tabs.List>

      {activeTab === "basic" && (
        <div className={styles.formTabContentExpanding}>
          <Flex direction="column" gap="3" style={{ flexShrink: 0 }}>
            <Flex direction="column" gap="1">
              <Text size="1" weight="medium">
                Title
              </Text>
              <TextField.Root
                size="1"
                value={title}
                onChange={(e) => patch(["title"], e.target.value)}
                placeholder="Display name"
              />
            </Flex>

            <Flex direction="column" gap="1">
              <Text size="1" weight="medium">
                Description
              </Text>
              <TextField.Root
                size="1"
                value={description}
                onChange={(e) => patch(["description"], e.target.value)}
                placeholder="Brief description"
              />
            </Flex>

            <Flex align="center" gap="2">
              <Switch
                size="1"
                checked={specific}
                onCheckedChange={(checked) => patch(["specific"], checked)}
              />
              <Text size="1">Internal Only</Text>
              <Text size="1" color="gray">
                (hide from mode selector)
              </Text>
            </Flex>
          </Flex>

          <div className={styles.expandingField}>
            <Text size="1" weight="medium">
              System Prompt
            </Text>
            <TextArea
              value={prompt}
              onChange={(e) => patch(["prompt"], e.target.value)}
              placeholder="System prompt for this mode..."
              className={styles.promptTextareaExpand}
            />
            <Text size="1" color="gray">
              Supports: %PROJECT_TREE%, %WORKSPACE_INFO%, %ARGS%, etc.
            </Text>
          </div>
        </div>
      )}

      {activeTab === "tools" && (
        <div className={styles.formTabContent}>
          <StringListEditor
            value={tools}
            onChange={(t) => patch(["tools"], t)}
            label="Available Tools"
            placeholder="Add tool..."
            suggestions={availableTools}
          />

          <RulesTableEditor
            value={toolConfirmRules}
            onChange={(rules) => patch(["tool_confirm", "rules"], rules)}
            label="Tool Confirmation Rules"
          />
        </div>
      )}

      {activeTab === "llm" && (
        <div className={styles.formTabContent}>
          <Flex direction="column" gap="3">
            <ModelTypeSection
              title="Default Model"
              typeKey="default"
              config={modelDefaultsDefault}
              groupedModels={groupedModels}
              onPatch={patch}
            />
            <ModelTypeSection
              title="Light Model"
              typeKey="light"
              config={modelDefaultsLight}
              groupedModels={groupedModels}
              onPatch={patch}
            />
            <ModelTypeSection
              title="Thinking Model"
              typeKey="thinking"
              config={modelDefaultsThinking}
              groupedModels={groupedModels}
              onPatch={patch}
            />
          </Flex>
        </div>
      )}

      {activeTab === "advanced" && (
        <div className={styles.formTabContent}>
          <Flex direction="column" gap="2">
            <Text size="1" weight="medium">
              Thread Defaults
            </Text>
            <Flex gap="3" wrap="wrap">
              <Flex align="center" gap="1">
                <Switch
                  size="1"
                  checked={
                    typeof threadDefaults.include_project_info === "boolean"
                      ? threadDefaults.include_project_info
                      : false
                  }
                  onCheckedChange={(checked) =>
                    patch(
                      ["thread_defaults", "include_project_info"],
                      checked || undefined,
                    )
                  }
                />
                <Text size="1">Project Info</Text>
              </Flex>
              <Flex align="center" gap="1">
                <Switch
                  size="1"
                  checked={
                    typeof threadDefaults.checkpoints_enabled === "boolean"
                      ? threadDefaults.checkpoints_enabled
                      : false
                  }
                  onCheckedChange={(checked) =>
                    patch(
                      ["thread_defaults", "checkpoints_enabled"],
                      checked || undefined,
                    )
                  }
                />
                <Text size="1">Checkpoints</Text>
              </Flex>
              <Flex align="center" gap="1">
                <Switch
                  size="1"
                  checked={
                    typeof threadDefaults.auto_approve_editing_tools ===
                    "boolean"
                      ? threadDefaults.auto_approve_editing_tools
                      : false
                  }
                  onCheckedChange={(checked) =>
                    patch(
                      ["thread_defaults", "auto_approve_editing_tools"],
                      checked || undefined,
                    )
                  }
                />
                <Text size="1">Auto Approve Editing</Text>
              </Flex>
              <Flex align="center" gap="1">
                <Switch
                  size="1"
                  checked={
                    typeof threadDefaults.auto_approve_dangerous_commands ===
                    "boolean"
                      ? threadDefaults.auto_approve_dangerous_commands
                      : false
                  }
                  onCheckedChange={(checked) =>
                    patch(
                      ["thread_defaults", "auto_approve_dangerous_commands"],
                      checked || undefined,
                    )
                  }
                />
                <Text size="1">Auto Approve Dangerous</Text>
              </Flex>
            </Flex>
          </Flex>

          <Flex direction="column" gap="1">
            <Text size="1" weight="medium">
              Base Mode
            </Text>
            <TextField.Root
              size="1"
              value={base ?? ""}
              onChange={(e) => patch(["base"], e.target.value || undefined)}
              placeholder="Inherit from (e.g., agent)"
            />
          </Flex>

          <StringListEditor
            value={matchModels ?? []}
            onChange={(models) =>
              patch(["match_models"], models.length > 0 ? models : undefined)
            }
            label="Match Models"
            placeholder="Model pattern..."
          />

          <Flex gap="2" wrap="wrap">
            <Flex direction="column" gap="1" style={{ minWidth: 80 }}>
              <Text size="1">UI Order</Text>
              <TextField.Root
                size="1"
                type="number"
                value={typeof ui.order === "number" ? ui.order.toString() : ""}
                onChange={(e) =>
                  patch(
                    ["ui", "order"],
                    e.target.value ? parseInt(e.target.value, 10) : undefined,
                  )
                }
                placeholder="Order"
              />
            </Flex>
          </Flex>

          <StringListEditor
            value={safeArray(ui.tags, isString)}
            onChange={(tags) => patch(["ui", "tags"], tags)}
            label="UI Tags"
            placeholder="Add tag..."
          />
        </div>
      )}
    </Tabs.Root>
  );
};
