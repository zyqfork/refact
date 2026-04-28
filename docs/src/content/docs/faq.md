---
title: FAQ
description: Frequently asked questions about local/BYOK Refact.
---

## Do I need a Refact account?

No. Refact runs locally and uses BYOK or local providers.

## Which providers can I use?

You can configure BYOK providers such as OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, Groq, xAI, and custom OpenAI-compatible endpoints. You can also use local runtimes such as Ollama, LM Studio, and vLLM.

## How am I billed?

Refact does not bill model usage. Billing, quotas, and rate limits are controlled by the provider or local runtime you configure.

## Does Refact send product usage reports?

No. Refact sends requests only to providers and integrations you configure.

## Where are model capabilities stored?

The engine bundles a local JSON model capability registry and uses configured provider data at runtime.
