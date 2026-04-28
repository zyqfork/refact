---
title: Quickstart
description: Get started with Refact as a BYOK/local-only coding assistant.
---

Refact runs locally in your IDE and connects only to providers you configure. There is no hosted account, managed Refact inference runtime, team workspace, or app-level balance.

## 1. Install Refact

- [VS Code](/installation/vs-code/)
- [JetBrains IDEs](/installation/jetbrains/)

## 2. Configure A Provider

Open **Provider Setup** in the Refact UI and add at least one provider:

- BYOK providers such as OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, Groq, xAI, or custom OpenAI-compatible endpoints.
- Local providers such as Ollama, LM Studio, or vLLM.

Refact uses your provider credentials directly from your local configuration. Provider billing, quotas, and availability are controlled by that provider.

## 3. Choose Defaults

After adding a provider, open **Default Models** and select models for chat, light chat, thinking, buddy, and code completion as needed.

## 4. Start Coding

Use chat, agent modes, code completion, integrations, and local knowledge features from your IDE. Usage statistics are local token/provider-cost summaries only.

## Privacy

Requests are sent only to the model providers or local runtimes you configure.
