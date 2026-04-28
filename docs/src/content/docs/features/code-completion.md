---
title: Code Completion
description: Code completion with local/BYOK providers.
---

Refact provides code completion from the local engine through the provider you configure.

## Setup

1. Add a completion-capable provider in **Provider Setup**.
2. Open **Default Models** and select a completion model.
3. Keep privacy settings aligned with the files you want Refact to use for context.

## Providers

Completion can use BYOK providers, custom OpenAI-compatible endpoints, or local runtimes such as Ollama, LM Studio, and vLLM when they expose a compatible completion model.

## Privacy

Completion requests are sent only to the configured provider or local runtime. Refact does not send snippet telemetry.
