---
title: Supported Models
description: How model support works in local/BYOK Refact.
---

Refact discovers models from configured providers and combines them with a bundled local model-capabilities registry in the engine.

## Provider Families

Refact can work with:

- OpenAI and OpenAI-compatible APIs.
- Anthropic.
- Google Gemini.
- DeepSeek.
- OpenRouter.
- Groq.
- xAI.
- Ollama.
- LM Studio.
- vLLM.
- Custom endpoints configured by the user.

## Capabilities

The bundled registry describes model context size, tool support, vision support, reasoning options, tokenizer hints, and related runtime limits. Provider APIs can also supply available model lists and provider-specific metadata.

## Costs

Refact does not bill model usage. Any cost shown in the UI is a local estimate based on provider pricing metadata where available. Your provider controls actual billing, quotas, and rate limits.

## Local Models

For local runtimes, ensure the model is already available in Ollama, LM Studio, vLLM, or your compatible endpoint. If a model is not in the bundled capability registry, configure custom model details in the provider settings.
