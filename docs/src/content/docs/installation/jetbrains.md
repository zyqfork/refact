---
title: JetBrains
description: Install and configure Refact for JetBrains IDEs.
---

## Install

Install the Refact plugin from JetBrains Marketplace or from a local plugin build.

## First Run

Open the Refact tool window. The first-run screen opens **Provider Setup**.

1. Add a BYOK provider such as OpenAI, Anthropic, Gemini, DeepSeek, OpenRouter, or a custom OpenAI-compatible endpoint.
2. Or add a local provider such as Ollama, LM Studio, or vLLM.
3. Pick default models in **Default Models**.

No hosted login, managed inference URL, legacy Refact server, team workspace, or Refact-issued API key is required.

## Local Engine

The plugin starts the local `refact-lsp` engine automatically and communicates with it over localhost.
