import { SupportsReasoningStyle } from "../../../../../services/refact";
import { BEAUTIFUL_PROVIDER_NAMES } from "../../../constants";

export function isSupportsReasoningStyle(
  data: string | null,
): data is SupportsReasoningStyle {
  return (
    data === "openai" ||
    data === "anthropic_budget" ||
    data === "anthropic_effort" ||
    data === "deepseek" ||
    data === "xai" ||
    data === "qwen" ||
    data === "gemini" ||
    data === "kimi" ||
    data === "zhipu" ||
    data === "mistral" ||
    data === null
  );
}

export function extractHumanReadableReasoningType(
  reasoningType: string | null,
) {
  if (!isSupportsReasoningStyle(reasoningType)) return null;
  if (!reasoningType) return null;

  // Reasoning type is a backend capability string, not a provider name.
  // Keep it user-friendly for the model cards.
  if (reasoningType === "anthropic_budget")
    return "Anthropic (Thinking tokens)";
  if (reasoningType === "anthropic_effort") return "Anthropic (Effort)";

  const maybeReadableReasoningType = BEAUTIFUL_PROVIDER_NAMES[reasoningType];

  return maybeReadableReasoningType
    ? maybeReadableReasoningType
    : reasoningType;
}
