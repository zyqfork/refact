---
title: Context
description: How Refact uses local project context.
---

Refact uses local project context to improve chat, agent workflows, and code completion.

## Sources

- Open files and selected snippets from the IDE.
- Project tree and file content allowed by privacy settings.
- Local AST and vector indexes.
- Project trajectories, tasks, and knowledge entries.

## Control

You control which files can be used through privacy settings and provider configuration. Context is sent only to the provider or local runtime selected for the request.
