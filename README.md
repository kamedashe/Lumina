# Lumina 💎 — Agentic Desktop Assistant

**Lumina** is an agentic AI assistant for your desktop. Connect any LLM provider — local or cloud — stream responses in real time, and let the model act on your system through native tool-calling: files, processes, and semantic search over your own documents.

---

## ✨ Features

### 🌐 Multi-Provider
One interface, any model. API keys are stored locally and every request goes through the Rust backend — nothing is proxied through a third party.

- **Ollama** — local models (Llama 3, Mistral, Gemma), no API key needed
- **OpenAI-compatible** — OpenAI, OpenRouter, Groq, LM Studio, vLLM, or any endpoint with a custom `base_url`
- **Anthropic** — Claude via the Messages API
- **Google Gemini** — `generateContent` with function calling

### 🤖 Agentic Tool-Calling
A real agent loop built on each provider's native tool-calling API — not text parsing. The model requests a tool → Rust executes it → the result is fed back to the model → the loop continues until a final answer. Every tool call appears in the chat as an expandable badge showing the raw result.

Available tools:

| Tool | Purpose |
|---|---|
| `list_files` | List files and folders in a directory |
| `read_file` | Read the contents of a text file |
| `write_file` | Create or overwrite a file |
| `get_current_dir` | Get the current working directory |
| `list_processes` | List running system processes |
| `search_documents` | Semantic search over attached documents |

### 📚 RAG with a Pluggable Vector Store
Indexes PDF, TXT, MD, JSON, and source code as chunked embeddings. Search is scoped to the files you attach, and re-indexing replaces stale chunks instead of duplicating them. Embeddings run through your provider, with an automatic local-Ollama fallback for Anthropic (which has no embeddings endpoint).

Three interchangeable backends, selectable in Settings → Storage:

| Backend | Trade-off |
|---|---|
| **sqlite-vec** (default) | Fully local. Vectors are stored as compact binary blobs and distances computed by the extension's SIMD code. Note this is still an **exhaustive scan, not ANN** — [ANN indexes are planned but not shipped](https://github.com/asg017/sqlite-vec/issues/25) — it simply has a far better constant factor than the naive backend. |
| **SQLite** | Dependency-free fallback. Embeddings are stored as JSON and re-parsed on every query, which dominates the cost. Kept for existing indexes and as an escape hatch. |
| **Pinecone** (serverless) | Genuine managed ANN that stays fast at scale and survives a reinstall. **Chunk text is uploaded**, since it lives in vector metadata — don't point it at confidential documents. |

> **sqlite-vec note:** `vec0` fixes the vector dimension at table creation, so switching embedding models rebuilds the index — re-attach your documents. TEXT metadata columns support only `=` / `!=` (no `IN`), so scoping a search to several attached files issues one KNN query per file and merges the results.

> **Pinecone note:** serverless indexes don't support delete-by-metadata-filter, so chunk IDs are structured as `{path_hash}#{chunk_index}` and re-indexing deletes via list-by-prefix. Index dimension must match your embedding model (`nomic-embed-text` = 768, `text-embedding-3-small` = 1536); the app checks this and reports a mismatch instead of letting Pinecone return an opaque 400.

### 🕸️ LangGraph Search (optional)
Single-shot retrieval embeds the raw conversational question — noise words and all — and takes whatever cosine similarity returns. Enabling **Settings → Storage → LangGraph search** routes document search through a corrective-RAG graph instead:

```
START → plan → retrieve → grade → (enough?) → compose → END
                  ↑                    │
                  └───────  refine  ←──┘
```

`plan` rewrites the question into a focused search query, `grade` filters out chunks that scored well on cosine but aren't actually relevant, and if too little survives, `refine` reformulates and the graph loops back for a second pass. State uses an `operator.add` reducer so results accumulate across passes rather than overwrite.

**The split that makes this work:** the graph does orchestration only — every piece of I/O stays in Rust. When a node needs retrieval or an LLM call it emits a callback over stdio and the host answers. So the Python side holds no API keys, no provider clients, and no knowledge of which of the three vector backends is active; it inherits all of them for free.

Costs 2–3 extra LLM calls per search, so it's **off by default**. Requires Python with LangGraph — any failure (no Python, missing dependency, timeout) falls back to direct vector search rather than breaking the turn.

```bash
pip install -r sidecar/requirements.txt
```

### ⚡ Streaming
Responses stream token by token over a Tauri IPC Channel, with SSE and NDJSON parsed on the Rust side. Agent status indicators show what the model is doing while it works.

### 🔌 Plugin System
A secure **QuickJS** sandbox for running JavaScript isolated inside the Rust environment. See [PLUGINS.md](PLUGINS.md).

### 🎨 Modern UI
A clean, restrained interface: light and dark themes driven by CSS variables, native Acrylic backdrop, and proper Markdown typography with code blocks and tables.

---

## 🛠 Tech Stack

- **Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/) — provider clients, agent loop, RAG, streaming
- **Frontend:** [React 19](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) + [Vite 6](https://vitejs.dev/)
- **Agent orchestration (optional):** [LangGraph](https://github.com/langchain-ai/langgraph) in a Python sidecar — corrective RAG with query rewriting and relevance grading
- **Vector store:** [sqlite-vec](https://github.com/asg017/sqlite-vec) / [rusqlite (SQLite)](https://github.com/rusqlite/rusqlite) locally, or [Pinecone](https://www.pinecone.io/) serverless in the cloud
- **JS Engine:** [rquickjs (QuickJS)](https://github.com/DelSkayn/rquickjs) — plugin sandbox
- **Styling:** [Tailwind CSS](https://tailwindcss.com/) with CSS-variable theming

---

## 🚀 Quick Start

### Requirements

1. **Node.js** (LTS)
2. **Rust** and Cargo
3. At least one provider:
   - Local: [Ollama](https://ollama.com/) or LM Studio
   - Cloud: an API key for OpenAI, OpenRouter, Groq, Anthropic, or Gemini
4. For RAG with local embeddings: `ollama pull nomic-embed-text`
5. Optional, for LangGraph search: Python 3.10+ and `pip install -r sidecar/requirements.txt`

### Installation

```bash
git clone https://github.com/kamedashe/Lumina.git
cd Lumina

npm install

npm run tauri dev
```

The local Ollama preset is active on first launch. To add a cloud provider, open Settings (⚙), pick a preset, paste your API key, and click **Models** to verify the connection and load the available model list.

---

## 📄 Project Structure

```
src-tauri/src/
  providers/        # LLM clients: ollama, openai, anthropic, gemini + SSE/NDJSON parser
  vector/           # Vector store backends: sqlite_vec, sqlite (local), pinecone (cloud)
  sidecar.rs        # Bridge to the LangGraph process; serves its retrieve/llm callbacks
  tools.rs          # Agent tool definitions and local execution
  rag.rs            # Text extraction, chunking, search orchestration
  main.rs           # Agent loop, Tauri commands, streaming channel
src/
  components/       # ChatMessage, ProviderSettings, UI primitives
  services/         # Frontend bridges to Tauri commands
  types/            # Shared types mirroring the Rust structs
sidecar/
  graph.py          # LangGraph corrective-RAG graph (orchestration only)
  test_graph.py     # Graph logic against a stubbed host
  test_protocol.py  # Real subprocess over stdio, as Rust drives it
PLUGINS.md          # Plugin system documentation
```

---

## 📄 License

MIT License. Built with love for open source. 🦀💎
