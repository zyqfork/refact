---
title: Privacy
description: Privacy model for local/BYOK Refact.
---

Refact is BYOK/local-only.

## What Leaves Your Machine

Only requests required by providers or integrations you explicitly configure leave your machine. Examples:

- Chat/completion requests to configured BYOK model providers.
- Requests to local runtimes such as Ollama, LM Studio, or vLLM when those endpoints are local.
- Integration calls you enable, such as GitHub or MCP servers.

## What Refact Does Not Send

- No coin, account, team, or survey data.
- No managed Refact runtime capability fetches.

## Local Data

Project data such as trajectories, tasks, knowledge, and local usage summaries are stored in project/user Refact directories. You control provider configuration and credentials locally.
