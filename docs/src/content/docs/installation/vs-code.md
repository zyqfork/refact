---
title: VS Code
description: Install and configure Refact for VS Code.
---

## Install

Install the Refact extension from the VS Code Marketplace or from a local extension build.

## First Run

Open the Refact sidebar. The first-run screen opens **Provider Setup**.

1. Add a BYOK provider such as OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, or a custom OpenAI-compatible endpoint.
2. Or add a local provider such as Ollama, LM Studio, or vLLM.
3. Pick default models in **Default Models**.

No hosted account, managed inference URL, legacy Refact server, or Refact-issued API key is required.

## Local Engine

The extension starts the local `refact-lsp` engine automatically and communicates with it over localhost.
