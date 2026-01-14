# Refact Agent Engine - Developer Guide

**Last Updated**: January 2025  
**Version**: 0.10.30  
**Repository**: https://github.com/smallcloudai/refact/tree/main/refact-agent/engine

---

## 📋 Table of Contents

1. [Project Overview](#project-overview)
2. [Architecture](#architecture)
3. [Build & Development](#build--development)
4. [Chat System](#chat-system)
5. [Tools System](#tools-system)
6. [HTTP API](#http-api)
7. [AST System](#ast-system)
8. [Vector Database (VecDB)](#vector-database-vecdb)
9. [Memory & Knowledge](#memory--knowledge)
10. [Integrations](#integrations)
11. [Testing](#testing)
12. [Configuration](#configuration)
13. [Background Tasks](#background-tasks)
14. [Git Integration](#git-integration)
15. [Code Completion](#code-completion)
16. [Voice & Multimodal](#voice--multimodal)

---

## Project Overview

### What is Refact Agent Engine?

Refact Agent Engine (`refact-lsp`) is a **self-contained AI coding agent** that serves as both an HTTP server and LSP (Language Server Protocol) server. It provides:

- **Real-time streaming chat** with tool execution and agent capabilities
- **Code completion** with Fill-In-the-Middle (FIM) and RAG
- **AST indexing** for 8 programming languages (C++, Python, Java, Kotlin, JavaScript, Rust, TypeScript)
- **Vector database** for semantic code search
- **Memory system** that learns from conversations
- **40+ tools** for file operations, web browsing, shell commands, databases, Docker
- **Integration framework** for external services (GitHub, GitLab, Chrome, PostgreSQL, MySQL, etc.)
- **Task management** with autonomous agents

### Key Characteristics

- **Single binary**: No external dependencies except optional voice models
- **Multi-modal**: Supports text, images, and voice transcription
- **Privacy-first**: BYOK (Bring Your Own Keys), local-first processing
- **Extensible**: YAML-based configuration, plugin integrations
- **Production-ready**: Comprehensive testing, telemetry, graceful shutdown

### Tech Stack

| Component | Technology |
|-----------|------------|
| **Language** | Rust (async/tokio) |
| **HTTP Server** | Axum + Tower |
| **LSP** | tower-lsp |
| **AST** | tree-sitter (6 languages) |
| **Vector DB** | SQLite + vec0 extension |
| **Storage** | LMDB (Heed), SQLite, JSON files |
| **AI/ML** | tokenizers, rmcp (SmallCloudAI SDK) |
| **Git** | git2 (libgit2) |
| **Browser** | headless_chrome |
| **Voice** | whisper-rs (optional) |

---

## Architecture

### High-Level Design

```
┌─────────────────────────────────────────────────────────────┐
│                    Client (IDE/CLI/Web)                      │
└────────────────────┬────────────────────────────────────────┘
                     │
        ┌────────────┴────────────┐
        │                         │
   HTTP Server              LSP Server
   (Axum :8001)            (tower-lsp)
        │                         │
        └────────────┬────────────┘
                     │
        ┌────────────▼────────────┐
        │   GlobalContext (Arc)    │
        │  - Capabilities          │
        │  - Chat Sessions         │
        │  - AST Database          │
        │  - Vector Database       │
        │  - Integrations          │
        │  - Memory/Knowledge      │
        └────────────┬────────────┘
                     │
    ┌────────────────┼────────────────┐
    │                │                │
┌───▼───┐      ┌────▼────┐      ┌───▼────┐
│  AST  │      │ VecDB   │      │  Git   │
│Indexer│      │ Thread  │      │ Shadow │
└───────┘      └─────────┘      └────────┘
```

### Core Modules

```
src/
├── main.rs                    # Entry point, server initialization
├── global_context.rs          # Shared state (Arc<RwLock<GlobalContext>>)
├── http/                      # HTTP server (Axum routes)
│   └── routers/v1/           # 50+ API endpoints
├── lsp.rs                     # LSP server (tower-lsp)
├── chat/                      # Chat system (16 files, ~7000 LOC)
│   ├── session.rs            # ChatSession state machine
│   ├── queue.rs              # Command queue processing
│   ├── generation.rs         # LLM streaming
│   ├── tools.rs              # Tool execution
│   └── trajectories.rs       # Persistence
├── tools/                     # 40+ tools (file_edit/, search, web, etc.)
├── ast/                       # Tree-sitter AST indexing
├── vecdb/                     # SQLite vector database
├── integrations/              # External service integrations
├── agentic/                   # AI agents (commit msgs, edits)
├── knowledge_graph/           # Memory & knowledge system
├── git/                       # Git operations & checkpoints
├── scratchpads/               # Code completion adapters
├── postprocessing/            # Output filtering & truncation
├── voice/                     # Whisper transcription (optional)
├── tasks/                     # Task board management
├── telemetry/                 # Usage tracking
└── yaml_configs/              # Configuration system
```

### Key Architectural Patterns

1. **Async Runtime**: Tokio multi-threaded with full features (fs, io, process, signal)
2. **Shared Mutable State**: `Arc<RwLock<GlobalContext>>` for central coordination
3. **Event-Driven Chat**: SSE (Server-Sent Events) for real-time updates
4. **Background Tasks**: Separate threads for indexing, vectorization, cleanup
5. **Tool-Based Agents**: OpenAI-compatible tool calling with confirmation gates
6. **Shadow Git Repos**: Isolated workspace snapshots for safe operations
7. **YAML-Driven Config**: Models, providers, integrations, prompts all configurable

---

## Build & Development

### Prerequisites

- **Rust**: 1.70+ (uses 2021 edition)
- **System Libraries**: OpenSSL, libclang (for tree-sitter)
- **Optional**: Docker (for integrations), Chrome (for browser tools)

### Quick Start

```bash
# Clone repository
git clone https://github.com/smallcloudai/refact
cd refact/refact-agent/engine

# Build release binary
cargo build --release

# Run server
./target/release/refact-lsp --http-port 8001 --logs-stderr

# Run with voice support (downloads Whisper model on first use)
cargo build --release --features voice
```

### Development Build

```bash
# Debug build (faster compilation, larger binary)
cargo build

# Run tests
cargo test --lib
cargo test --doc

# Run specific test
cargo test test_chat_session

# Check without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

### Build Configuration

**Cargo.toml Features:**
```toml
[features]
default = ["voice"]
voice = ["whisper-rs", "symphonia", "rubato"]
```

**Release Profile** (optimized for size):
```toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true             # Link-time optimization
strip = true           # Strip symbols
codegen-units = 1      # Single codegen unit
```

### Cross-Compilation

**Supported Targets:**
- `x86_64-unknown-linux-gnu` (default)
- `aarch64-unknown-linux-gnu` (ARM64)
- `x86_64-pc-windows-msvc` (Windows)
- `x86_64-apple-darwin` (macOS)

```bash
# Install cross
cargo install cross

# Build for ARM64 Linux
cross build --target aarch64-unknown-linux-gnu --release

# Build for Windows
cross build --target x86_64-pc-windows-msvc --release
```

### Docker Build

```bash
# Build LSP server in Docker
docker build -f docker/lsp-release.Dockerfile -t refact-lsp .

# Build Chrome integration
docker build -f docker/chrome/Dockerfile -t refact-chrome docker/chrome/

# Run
docker run -p 8001:8001 refact-lsp
```

### Python Binding

The engine includes Python bindings for CLI usage:

```bash
cd python_binding_and_cmdline

# Install in development mode
pip install -e .

# Use CLI
refact --help
refact chat "Explain this code"
```

### Project Structure

**Key Directories:**
- `src/` - Rust source code (~70 modules)
- `tests/` - Python integration tests (~35 files)
- `examples/` - Usage examples (HTTP, LSP, tools)
- `docker/` - Dockerfiles for builds and integrations
- `python_binding_and_cmdline/` - Python CLI wrapper

**Configuration Locations:**
- `~/.config/refact/` - User configuration
- `~/.cache/refact/` - Cache, telemetry, shadow repos
- `.refact/` - Project-specific (trajectories, knowledge, tasks)

---

## Chat System

### Overview

The chat system (`src/chat/`) implements a **stateful, event-driven architecture** with:
- **Real-time SSE streaming** for live updates
- **Command queue** for concurrent operations
- **Trajectory persistence** for conversation history
- **Tool execution loop** with approval gates
- **OpenAI compatibility** layer

### Architecture

**16 Core Files (~7000 LOC):**

| File | Purpose | LOC |
|------|---------|-----|
| `session.rs` | ChatSession state machine | 976 |
| `queue.rs` | Command queue processing | 595 |
| `handlers.rs` | HTTP endpoint handlers | 190 |
| `prepare.rs` | Message preparation & validation | 492 |
| `generation.rs` | LLM streaming integration | 491 |
| `tools.rs` | Tool execution & approval | 326 |
| `trajectories.rs` | Trajectory persistence & loading | 1198 |
| `openai_convert.rs` | OpenAI format conversion | 535 |
| `openai_merge.rs` | Streaming delta merge | 279 |
| `content.rs` | Message content utilities | 330 |
| `types.rs` | Data structures & events | 489 |
| `tests.rs` | Unit tests | 1086 |
| `history_limit.rs` | Token compression pipeline | (renamed) |
| `prompts.rs` | System prompts | (renamed) |
| `system_context.rs` | Context generation | (moved) |

### ChatSession State Machine

```rust
pub struct ChatSession {
    chat_id: String,
    thread: ThreadParams,           // Model, mode, title, task_meta
    messages: Vec<ChatMessage>,     // Conversation history
    runtime: RuntimeState,          // Current state + queue info
    draft_message: Option<ChatMessage>,  // Streaming response
    command_queue: VecDeque<CommandRequest>,
    event_tx: broadcast::Sender<EventEnvelope>,  // SSE events
    abort_flag: Arc<AtomicBool>,
    trajectory_dirty: bool,
    trajectory_version: u64,
}
```

**States:**
```
Idle → Generating → ExecutingTools → Paused → Idle
  ↓                      ↓              ↓
  └─────────────────────┴──────────────┘
         (loop until no more tools)
```

| State | Description |
|-------|-------------|
| `Idle` | Ready for commands |
| `Generating` | LLM streaming response |
| `ExecutingTools` | Running tool calls |
| `Paused` | Waiting for user approval |
| `WaitingIde` | Waiting for IDE tool results |
| `Error` | Failed generation |

### SSE Event System

**Subscription Endpoint:** `GET /v1/chats/subscribe?chat_id={id}`

**Event Format:**
```
data: {"type":"snapshot","seq":0,"thread":{...},"runtime":{...},"messages":[...]}\n\n
data: {"type":"stream_started","seq":1,"msg_id":5}\n\n
data: {"type":"stream_delta","seq":2,"ops":[{"op":"append_content","value":"Hello"}]}\n\n
data: {"type":"stream_finished","seq":3,"usage":{"total_tokens":50}}\n\n
```

**Key Event Types:**

| Event | Purpose |
|-------|---------|
| `Snapshot` | Full state sync (sent on connect) |
| `StreamStarted` | AI response beginning |
| `StreamDelta` | Incremental content updates |
| `StreamFinished` | AI response complete with usage |
| `MessageAdded` | New message in thread |
| `MessageUpdated` | Message content changed |
| `MessageRemoved` | Message deleted |
| `ThreadUpdated` | Thread metadata changed |
| `RuntimeUpdated` | Runtime state changed |
| `PauseRequired` | Tool confirmation needed |
| `PauseCleared` | Confirmation resolved |
| `IdeToolRequired` | IDE tool execution needed |
| `Ack` | Command acknowledgment |

**Sequence Numbers:**
- Every event has monotonic `seq: u64`
- Gap detection triggers reconnect for fresh snapshot
- Prevents missed events in unreliable networks

### Command Types

**Sent via:** `POST /v1/chats/{chat_id}/commands`

```rust
enum ChatCommand {
    UserMessage {content: Value, attachments: Vec<Value>},
    RetryFromIndex {index: usize, content: Value},
    SetParams {patch: Value},       // Update thread params
    Abort {},
    ToolDecision {tool_call_id: String, accepted: bool},
    ToolDecisions {decisions: Vec<ToolDecisionItem>},
    IdeToolResult {tool_call_id: String, content: String, tool_failed: bool},
    UpdateMessage {message_id: String, content: Value, regenerate: bool},
    RemoveMessage {message_id: String, regenerate: bool},
    Regenerate {},
}
```

**Command Flow:**
```
Client → POST /v1/chats/{id}/commands → Queue → Process → SSE Events
```

### Delta Operations

Streaming updates use fine-grained delta operations:

```rust
enum DeltaOp {
    AppendContent {text: String},
    AppendReasoning {text: String},
    SetToolCalls {tool_calls: Vec<Value>},
    SetThinkingBlocks {blocks: Vec<Value>},
    AddCitation {citation: Value},
    SetUsage {usage: Value},
    MergeExtra {extra: Map<String, Value>},
}
```

### Trajectory Persistence

**Storage:** `.refact/trajectories/{chat_id}.json`

```json
{
  "id": "chat-abc123",
  "title": "Fix authentication bug",
  "created_at": "2024-12-25T10:00:00Z",
  "updated_at": "2024-12-25T10:45:00Z",
  "model": "gpt-4o",
  "mode": "AGENT",
  "tool_use": "agent",
  "messages": [...],
  "task_meta": {...},
  "version": 5
}
```

**Features:**
- Atomic writes (`.json.tmp` → `.json`)
- File watcher for external changes
- Auto-title generation via LLM
- Task-specific directories for task agents

### Message Preparation Flow

```
UserMessage → queue → process_command_queue()
    ↓
add_message() → emit(MessageAdded) → start_generation()
    ↓
start_generation():
1. start_stream() → draft_message + emit(StreamStarted)
2. run_llm_generation():
   a. Load tools by ChatMode
   b. Resolve model caps → effective_n_ctx
   c. Inject system/project context
   d. Knowledge enrichment (RAG for AGENT mode)
   e. prepare_chat_passthrough():
      - Adapt sampling (reasoning/thinking budgets)
      - History limit/fix
      - OpenAI conversion
   f. run_streaming_generation():
      - run_llm_stream() → StreamCollector
      - emit_stream_delta(DeltaOp)
      - Accumulate content/tool_calls/reasoning
3. finish_stream() → add_message(draft)
4. process_tool_calls_once() → Paused if approval needed
    ↓ Loop if more tools/generation needed
```

### Token Compression Pipeline

**7-Stage History Limit** (`history_limit.rs`):

```
Stage 0: Deduplicate context files (keep largest)
Stage 1: Compress old context files → hints
Stage 2: Compress old tool results → hints
Stage 3: Compress outlier messages
Stage 4: Drop entire conversation blocks
Stage 5: Aggressive compression (even recent)
Stage 6: Last resort - newest context
Stage 7: Ultimate fallback
```

**Result:** Always fits token budget or fails gracefully with clear error.

### OpenAI Compatibility

**Conversion** (`openai_convert.rs`):
- Internal `ChatMessage` → OpenAI `[{"role", "content"}]`
- Tool results: `role="tool"` with `tool_call_id`
- Thinking blocks preserved for Anthropic
- Multimodal content (images) supported
- Citations included

**Litellm Proxy:** Converts OpenAI → provider-native formats (Anthropic, etc.)

### Chat Modes

| Mode | Tool Use | Purpose |
|------|----------|---------|
| `NO_TOOLS` | None | Basic chat |
| `EXPLORE` | Quick tools | Context gathering |
| `AGENT` | Full agent | Autonomous task execution |
| `TASK_PLANNER` | Task tools | Kanban board management |
| `TASK_AGENT` | Task tools | Execute task cards |

### Key APIs

**Session Management:**
```rust
// Get or create session (loads from trajectory if exists)
pub async fn get_or_create_session_with_trajectory(
    chat_id: String,
    gcx: Arc<ARwLock<GlobalContext>>,
) -> Result<Arc<AMutex<ChatSession>>>

// Subscribe to session events (SSE)
pub fn subscribe(&self) -> broadcast::Receiver<ChatEvent>

// Add command to queue
pub async fn add_command(&mut self, req: CommandRequest) -> Result<()>
```

**HTTP Endpoints:**
- `POST /v1/chats/{id}/commands` - Send commands
- `GET /v1/chats/subscribe?chat_id={id}` - SSE subscription
- `POST /v1/chat` - Legacy stateless endpoint (backward compatible)

---

## Tools System

### Overview

The tools system (`src/tools/`) provides **40+ tools** for autonomous agent operations. Tools implement the `Tool` trait and are registered via `tools_list.rs`.

### Tool Categories

**1. Codebase Search (AST/Vector/Regex)**

| Tool | Purpose | Dependencies |
|------|---------|--------------|
| `search_symbol_definition` | Find AST definitions of symbols | `ast` |
| `tree` | Project file tree with sizes/lines | - |
| `cat` | Read files/images (multi-file, line ranges) | - |
| `search_pattern` | Regex search files/paths/text | - |
| `search_semantic` | Vector DB semantic search | `vecdb` |

**2. Codebase Change (Confirmation Required)**

| Tool | Purpose |
|------|---------|
| `create_textdoc` | Create new text file |
| `update_textdoc` | Simple string replacement |
| `update_textdoc_anchored` | Anchor-based editing |
| `update_textdoc_by_lines` | Line range replacement |
| `update_textdoc_regex` | Regex-based editing |
| `apply_patch` | Apply unified diffs |
| `undo_textdoc` | Undo recent changes |
| `rm` | Delete file/dir (recursive/dry-run) |
| `mv` | Move/rename files/dirs |

**3. Web**

| Tool | Purpose |
|------|---------|
| `web` | Fetch web pages (Jina Reader API) |
| `web_search` | Search the web (returns snippets) |
| `chrome` | Browser automation (navigate, screenshot, click) |

**4. System**

| Tool | Purpose |
|------|---------|
| `shell` | Execute shell commands (streaming) |
| `cmdline_*` | One-off CLI commands (user-defined) |
| `service_*` | Long-running services (user-defined) |

**5. Memory & Knowledge**

| Tool | Purpose |
|------|---------|
| `knowledge` | Search knowledge base + graph expansion |
| `create_knowledge` | Create memory entry |
| `search_trajectories` | Find relevant past conversations |
| `get_trajectory_context` | Load specific conversation context |

**6. Agent Tools**

| Tool | Purpose |
|------|---------|
| `subagent` | Spawn independent sub-agent |
| `strategic_planning` | Plan complex solutions |
| `deep_research` | Comprehensive web research |

### Tool Execution Flow

```
LLM suggests tool_call
    ↓
pause_required SSE event
    ↓
Confirmation popup shown (if needed)
    ↓
User approves/rejects
    ↓
POST /v1/chats/{id}/commands (tool_decision)
    ↓
Backend executes tool
    ↓
Result via SSE events
    ↓
AI continues with result
```

### Tool Trait

```rust
pub trait Tool: Send + Sync {
    fn tool_description(&self) -> ToolDesc;
    
    async fn tool_execute(
        &mut self,
        ccx: Arc<AMutex<AtCommandsContext>>,
        tool_call_id: String,
        args: HashMap<String, Value>,
    ) -> Result<(bool, Vec<ContextEnum>)>;
    
    fn tool_depends_on(&self) -> Vec<String> {
        vec![]  // e.g., ["ast"], ["vecdb"]
    }
}
```

### Tool Registration

**Discovery** (`tools_list.rs`):
```rust
pub fn get_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(ToolCat::default()),
        Box::new(ToolTree::default()),
        Box::new(ToolPatch::default()),
        // ... 40+ tools
    ]
}
```

**Integration Tools** (dynamic):
- Loaded from `integrations.d/*.yaml`
- MCP (Model Context Protocol) servers
- User-defined `cmdline_*` and `service_*` tools

### Confirmation System

**Safety Gates:**
- Destructive operations require approval
- Configurable per-tool via YAML
- Glob patterns for allow/deny lists

```yaml
confirmation:
  ask_user_default: ["*"]           # Ask for all by default
  deny_default: ["rm -rf /"]        # Always deny dangerous commands
```

### Subagent Tool

**Purpose:** Spawn independent agents for focused tasks

```rust
{
  "task": "Find all usages of function X",
  "expected_result": "List of files and line numbers",
  "tools": "search_symbol_definition,cat",
  "max_steps": "10"
}
```

**Features:**
- Independent context (doesn't see parent conversation)
- Tool restrictions
- Step limits
- Result synthesis

### Tool Output Postprocessing

**Intelligent Truncation** (`postprocessing/`):
- Token-aware line truncation
- AST-based prioritization (symbols > lines)
- Deduplication and merging
- Grep/top/bottom filtering
- Warnings for truncated content

### IDE Integration Tools

**Special Tools for IDE Communication:**

| Tool | Purpose |
|------|---------|
| `ide_open_file` | Open file in editor |
| `ide_paste_text` | Paste at cursor |
| `ide_get_active_file` | Get current file context |

**Communication:** Via postMessage (web) or LSP custom methods

### Tool Dependencies

Tools can declare dependencies on system capabilities:

```rust
fn tool_depends_on(&self) -> Vec<String> {
    vec!["ast".to_string()]  // Requires AST indexing
}
```

**Dependency Resolution:**
- Tools filtered based on available capabilities
- Graceful degradation if dependencies unavailable
- Clear error messages

### Tool Execution Context

```rust
pub struct AtCommandsContext {
    pub global_context: Arc<ARwLock<GlobalContext>>,
    pub chat_id: String,
    pub n_ctx: usize,
    pub top_n: usize,
    pub abort_flag: Arc<AtomicBool>,
    // ... other fields
}
```

**Provides:**
- Access to AST, VecDB, Git, integrations
- Token budgets
- Abort signals
- Telemetry hooks

---

## HTTP API

### Base URL

All endpoints under `/v1/` (base: `http://127.0.0.1:8001`)

### Core Endpoints

**Health & Capabilities:**
- `GET /v1/ping` - Health check
- `GET /v1/caps` - Server capabilities/models
- `GET /v1/graceful-shutdown` - Trigger shutdown

**Chat:**
- `POST /v1/chats/{id}/commands` - Send commands (queue)
- `GET /v1/chats/subscribe?chat_id={id}` - SSE subscription
- `POST /v1/chat` - Legacy stateless endpoint

**Code Completion:**
- `POST /v1/code-completion` - FIM completion (stream/non-stream)
- `POST /v1/code-lens` - Symbol definitions/usages per file

**Tools:**
- `GET /v1/tools` - List available tools
- `POST /v1/tools` - Update tool configurations
- `POST /v1/tools-check-if-confirmation-needed` - Check permissions
- `POST /v1/tools-execute` - Execute tools

**AST:**
- `POST /v1/ast-file-symbols` - AST symbols for file
- `POST /v1/ast-file-dump` - AST dump for file
- `GET /v1/ast-status` - AST indexing status

**VecDB:**
- `GET /v1/rag-status` - RAG/vector DB status
- `POST /v1/vecdb-search` - Semantic search

**Git:**
- `POST /v1/git-commit` - Git commits
- `POST /v1/checkpoints-preview` - Checkpoint restore preview
- `POST /v1/checkpoints-restore` - Restore checkpoint

**Integrations:**
- `GET /v1/integrations` - List integrations
- `POST /v1/integration-get` - Get integration config
- `POST /v1/integration-save` - Save integration config

**Knowledge:**
- `POST /v1/knowledge/update-memory` - Create/update memory
- `POST /v1/knowledge/delete-memory` - Delete memory
- `GET /v1/knowledge-graph` - Knowledge graph visualization

**Voice:**
- `POST /v1/voice/transcribe` - Full audio transcription
- `GET /v1/voice/stream/{session_id}` - SSE subscribe to session
- `POST /v1/voice/stream/{session_id}/chunk` - Add audio chunk

**Telemetry:**
- `POST /v1/telemetry-network` - Network events
- `POST /v1/telemetry-chat` - Chat events
- `POST /v1/snippet-accepted` - Snippet acceptance

### Middleware

```
Request → CORS (permissive) → Body Limit (15MB) → Telemetry → Handler
```

**CORS:** `CorsLayer::very_permissive()` - Allows all origins/methods

**Telemetry Middleware:**
- Logs request start/completion
- Skips spam endpoints (ping, rag-status)
- Captures errors for telemetry
- Timing: "--- HTTP /endpoint starts ---" / "completed Xms"

### Response Formats

**Success:**
```json
{
  "success": true,
  "data": {...}
}
```

**Error:**
```json
{
  "detail": "error message"
}
```

**Streaming:** Server-Sent Events (`text/event-stream`)
```
data: {"type":"event","seq":1,...}\n\n
```

---

## AST System

### Overview

The AST system (`src/ast/`) provides **multi-language code analysis** using tree-sitter parsers.

### Supported Languages

| Language | Extensions | Parser |
|----------|------------|--------|
| C/C++ | .cpp, .cc, .c, .h, .hpp | `CppParser` |
| Python | .py, .py3, .pyx | `PythonParser` (hybrid) |
| Java | .java | `JavaParser` |
| Kotlin | .kt, .kts | `KotlinParser` |
| JavaScript | .js, .jsx | `JSParser` |
| Rust | .rs | `RustParser` |
| TypeScript | .ts, .tsx | `TSParser` |

**Total:** 8 languages with full AST support

### Architecture

```
File Changes → AST Indexer Thread → Parse → Store in LMDB
                                              ↓
                                    Definitions + Usages
                                              ↓
                                    Connect Usages (Phase 2)
```

### Storage (LMDB via Heed)

**Key Prefixes:**

| Prefix | Format | Value | Purpose |
|--------|--------|-------|---------|
| `d\|` | `d\|full::path` | `AstDefinition` | Definitions |
| `c\|` | `c\|short::path ⚡ full::path` | `[]` | Fuzzy lookup |
| `u\|` | `u\|resolved::target ⚡ usage_loc` | `[uline]` | Back-links |
| `classes\|` | `classes\|parent ⚡ child` | `lang🔎Child` | Inheritance |
| `counters\|` | `counters\|defs/usages/docs` | `[i32]` | Stats |

### Symbol Types

```rust
enum SymbolType {
    Module,
    StructDeclaration,
    FunctionDeclaration,
    VariableDefinition,
    VariableUsage,
    FunctionCall,
    ImportDeclaration,
    CommentDefinition,
    ClassFieldDeclaration,
    TypeAlias,
    Unknown,
}
```

### Indexing Process

**Two-Phase:**
1. **Parse & Store**: Extract definitions/usages → store raw
2. **Link**: Resolve cross-references → connect usages to definitions

**Background Thread:**
- Queue: `IndexSet<String>` (file paths)
- Batch processing with stats every 1s
- Idle: `connect_usages()` (resolve cross-refs)
- Limits: `ast_max_files` (queue cap)

### Queries

```rust
// Get definitions for a file
definitions(path) -> Vec<AstDefinition>

// Get usages of a symbol
usages(path) -> Vec<AstUsage>

// Get type hierarchy
type_hierarchy(lang, klass) -> Vec<String>
```

### Skeletonizer

**Purpose:** Generate abbreviated code for embeddings

```rust
// Full function
fn calculate_total(items: Vec<Item>) -> f64 {
    items.iter().map(|i| i.price).sum()
}

// Skeleton
fn calculate_total(items: Vec<Item>) -> f64 { ... }
```

### Integration with Code Completion

**AST-RAG:**
- Find nearest usages/defs around cursor
- Extract context files
- Postprocess to fit token budget
- Render in model format (starcoder/qwen2.5/chat)

---

## Vector Database (VecDB)

### Overview

The VecDB system (`src/vecdb/`) provides **semantic code search** using SQLite with the `vec0` extension.

### Architecture

```
Files → Enqueue → Background Thread → Split → Cache Check → Embed → Store
                                                                      ↓
                                                            SQLite vec0 Tables
                                                                      ↓
                                                            Search (Cosine KNN)
```

### Storage

**Database:** `vecdb_model_<model_name>_<embedding_size>.sqlite`

**Tables:**
```sql
-- Vector table (one per workspace+timestamp)
CREATE VIRTUAL TABLE emb_<hash>_<timestamp> 
USING vec0(
    embedding float[EMBEDDING_SIZE] distance_metric=cosine,
    scope TEXT,
    +start_line INTEGER,
    +end_line INTEGER
);

-- Cache table (deduplication)
CREATE TABLE embeddings_cache (
    vector BLOB,
    window_text TEXT,
    window_text_hash TEXT
);
```

### Embedding Provider

**External HTTP API:**
- Configured via `EmbeddingModelRecord`
- Batch processing (`embedding_batch` size)
- Rate limiting: 1s sleep between batches
- Retry logic (3x, 100ms)

### File Splitting Strategies

| File Type | Splitter | Strategy |
|-----------|----------|----------|
| Trajectories (`.json`) | `TrajectoryFileSplitter` | 4 msgs/chunk, overlap 1 |
| Markdown (`.md`) | `MarkdownFileSplitter` | Headings → sections → char chunks |
| Code | `AstBasedFileSplitter` | Token window (n_ctx/2), AST chunks |

**Generic Logic:**
- Accumulate lines until token limit
- Paragraph-aware (empty lines trigger chunk)
- AST-enhanced: Symbol-aware subchunks
- Always add: Filename chunk (whole-file search)

### Search

**Query Flow:**
```
Query text → Embed → SQLite vec0 KNN → Post-filter → Re-rank → Results
```

**Ranking:**
- Cosine distance (via vec0)
- Reject if `distance >= rejection_threshold`
- Normalize: `usefulness = 100 - 75 * normalized_distance`

**Filters:**
- Optional `scope` (file path exact match)
- Top-K configurable

### Background Processing

**VecDB Thread:**
- Event-driven (enqueue files) + cooldown (10s)
- Processes queue: split → cache lookup → embed → store
- Status: `VecDbStatus` (queue, DB size, errors, states)

### Cleanup

- Keep 10 newest tables
- Drop tables >7 days old
- Migrate from legacy paths
- Schema upgrades (202406/202501)

---

## Memory & Knowledge

### Dual Memory System

**1. Memories** - Short-term semantic-searchable notes extracted from trajectories or manually created. Stored as Markdown with YAML frontmatter, indexed in VecDB.

**2. Knowledge Graph** - Long-term structured knowledge base with entities, relationships, auto-enrichment via LLM, and deprecation tracking.

### Memory Types

- **pattern**: Reusable code patterns/approaches
- **preference**: User preferences (style, tools, communication)
- **lesson**: What went wrong + fix
- **decision**: Architectural/design decisions
- **insight**: Codebase/project observations

### Trajectory Memory Extraction

Automatic process for abandoned trajectories (>2h idle, ≥10 messages): LLM analyzes conversation, extracts 3-10 memory items, saves to `.refact/trajectories/*/memories/`, indexes in VecDB.

---

## Integrations

### Supported Integrations

GitHub, GitLab, Chrome (headless), PostgreSQL, MySQL, Docker, Shell, cmdline_* (one-off), service_* (long-running), MCP stdio/SSE servers.

### Configuration

**Locations:** `.refact/integrations/*.yaml`, `~/.cache/refact/integrations/*.yaml`

**Integration Trait:**
```rust
pub trait IntegrationTrait {
    async fn integr_tools(&self, name: &str) -> Vec<Box<dyn Tool>>;
    fn integr_schema(&self) -> &'static str;
    async fn integr_settings_apply(&mut self, gcx, path, json);
}
```

---

## Testing

### Test Infrastructure

**Python Integration Tests** (~35 files): Live LSP server testing via HTTP API + SSE. Key: `test_chat_session_*.py` (8 files, ~3700 LOC).

**Rust Unit Tests**: `src/chat/tests.rs` (1402 lines), AST parser tests, ~50+ modules.

```bash
# Run tests
pytest tests/ -v -s
cargo test --lib
```

---

## Configuration

### Files

**User** (`~/.config/refact/`): `customization.yaml`, `privacy.yaml`, `indexing.yaml`, `providers.d/*.yaml`

**Project** (`.refact/`): `trajectories/`, `knowledge/`, `tasks/`

### System Prompts

Key prompts: `default`, `agentic_tools`, `exploration_tools`, `task_planner`, `task_agent`

Magic variables: `%ARGS%`, `%CODE_SELECTION%`, `%WORKSPACE_INFO%`, `%PROJECT_TREE%`

---

## Background Tasks

| Task | Interval | Purpose |
|------|----------|---------|
| telemetry | 1h | Send telemetry |
| git_shadow_cleanup | 24h | Remove old repos |
| knowledge_cleanup | 24h | Archive stale docs |
| vecdb_reload | 60s | Config changes |
| stuck_agents | 5min | Monitor tasks |

---

## Git Integration

### Shadow Repositories

Isolated workspace snapshots in `~/.cache/refact/shadow_git/{hash}/`. Chat-specific branches (`refact-{chat_id}`), checkpoint system, background cleanup.

### Checkpoints

```rust
create_workspace_checkpoint(gcx, prev, chat_id)
preview_changes_for_workspace_checkpoint(gcx, chat_id)
restore_workspace_checkpoint(gcx, chat_id)
```

---

## Code Completion

### FIM (Fill-In-the-Middle)

Model-specific adapters with PSM/SPM order support, bidirectional context, AST-RAG integration.

**Orders:**
- **PSM**: `<fim_prefix>before<fim_suffix>after<fim_middle>`
- **SPM**: `<fim_suffix>after<fim_prefix>before<fim_middle>`

### Postprocessing

Intelligent truncation: AST-based prioritization, dedup/merge, usefulness scoring (symbols > lines).

---

## Voice & Multimodal

### Voice

Whisper-based transcription (optional), streaming sessions, models: tiny to large-v3, formats: WAV/WebM/OGG/MP3.

**Endpoints:** `/v1/voice/transcribe`, `/v1/voice/stream/{id}`

### Multimodal

**Images**: Fully supported in chat (OpenAI format, token counting)
**Audio**: Transcription only (not as chat content)

---

## Performance

### Token Budgets

- Code completion: `n_ctx - max_new_tokens - rag`
- Chat: 7-stage compression pipeline
- Tools: AST prioritization

### Caching

- Completion: 500 entries (LRU)
- VecDB: Hash-based dedup
- Token counts: Unlimited

### Concurrency

- Tokio multi-threaded
- Rayon parallel indexing
- Queue limits: 100 commands/session

---

## Troubleshooting

**AST not indexing:** Check `ast_max_files`, blocklist, `/v1/ast-status`

**VecDB issues:** Verify embedding model, `/v1/rag-status`, SQLite vec0

**Chat not streaming:** Check SSE connection, sequence numbers

**Tools not executing:** Check dependencies, confirmation settings

**Logs:** `~/.cache/refact/logs/` (JSON/DEBUG, daily rotation)

---

## Resources

- **Repository**: https://github.com/smallcloudai/refact
- **Documentation**: https://docs.refact.ai
- **Discord**: https://discord.gg/refact
- **Issues**: https://github.com/smallcloudai/refact/issues

---

**Last Updated**: January 2025 | **Version**: 0.10.30





