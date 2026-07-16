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

### 📚 Local RAG
- Indexes PDF, TXT, MD, JSON, and source code as chunked embeddings in a local SQLite store
- Search is scoped to the files you attach, so context stays relevant
- Re-indexing replaces stale chunks instead of duplicating them
- Embeddings run through your provider, with an automatic local-Ollama fallback for Anthropic (which has no embeddings endpoint)

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
- **Database:** [rusqlite (SQLite)](https://github.com/rusqlite/rusqlite) — local vector store for RAG
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
  tools.rs          # Agent tool definitions and local execution
  rag.rs            # Document indexing and semantic search (SQLite)
  main.rs           # Agent loop, Tauri commands, streaming channel
src/
  components/       # ChatMessage, ProviderSettings, UI primitives
  services/         # Frontend bridges to Tauri commands
  types/            # Shared types mirroring the Rust structs
PLUGINS.md          # Plugin system documentation
```

---

## 📄 License

MIT License. Built with love for open source. 🦀💎
