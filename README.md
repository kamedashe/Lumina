# Lumina 💎 — Agentic Desktop Assistant

**Lumina** — агентный AI-ассистент для рабочего стола: подключается к любому LLM-провайдеру (локальному или облачному), стримит ответы в реальном времени и умеет работать с вашей системой через нативный tool-calling — файлы, процессы, семантический поиск по документам.

**Lumina** is an agentic desktop AI assistant: connect any LLM provider (local or cloud), stream responses in real time, and let the model act on your system through native tool-calling — files, processes, and semantic search over your documents.

---

## ✨ Features / Возможности

### 🌐 Multi-Provider / Мультипровайдерность
Один интерфейс — любые модели. Ключи хранятся локально, все запросы идут через Rust-бэкенд:
- **Ollama** — локальные модели (Llama 3, Mistral, Gemma), без ключа
- **OpenAI-совместимые** — OpenAI, OpenRouter, Groq, LM Studio, vLLM (любой `base_url`)
- **Anthropic** — Claude через Messages API
- **Google Gemini** — generateContent + function calling

One UI — any model. API keys never leave your machine; every request goes through the Rust backend:
- **Ollama** — local models, no key required
- **OpenAI-compatible** — OpenAI, OpenRouter, Groq, LM Studio, vLLM (any `base_url`)
- **Anthropic** (Claude) and **Google Gemini**

### 🤖 Agentic Tool-Calling / Агентный цикл
Настоящий tool-calling через нативные API провайдеров (не парсинг текста): модель запрашивает инструмент → Rust выполняет его → результат возвращается модели → цикл продолжается до финального ответа. Инструменты:
- `list_files` / `read_file` / `write_file` — работа с файловой системой
- `get_current_dir`, `list_processes` — системная осведомлённость
- `search_documents` — семантический поиск по прикреплённым документам

Real agentic loop via native provider tool-calling APIs (no text parsing): the model requests a tool → Rust executes it → the result goes back to the model → the loop continues until a final answer. Tool results are shown as expandable badges in the chat.

### 📚 Local RAG / Локальный RAG
- Индексация PDF, TXT, MD, JSON и исходного кода (чанки + эмбеддинги в SQLite)
- Поиск ограничивается прикреплёнными файлами; переиндексация не плодит дубликаты
- Эмбеддинги через провайдера (для Claude — автоматический фолбэк на локальный Ollama)

Chunked embeddings stored in local SQLite; search is scoped to attached files; re-indexing replaces stale chunks. Embeddings go through your provider (with an automatic local-Ollama fallback for Anthropic).

### ⚡ Streaming UI
Ответы стримятся токен за токеном через Tauri IPC Channel (SSE/NDJSON-парсинг на стороне Rust) — с индикатором статуса агента и мигающим курсором.

Token-by-token streaming over a Tauri IPC Channel (SSE/NDJSON parsed in Rust), with agent status indicators.

### 🔌 Plugin System / Плагины
Безопасная песочница **QuickJS** — выполнение JavaScript внутри Rust-окружения.

Secure **QuickJS** sandbox for running JavaScript inside the Rust environment.

### 🎨 Modern UI
Сдержанный современный интерфейс: светлая и тёмная темы, системный Acrylic-эффект, аккуратная типографика Markdown с блоками кода и таблицами.

Clean, restrained UI: light & dark themes, native Acrylic effect, proper Markdown typography with code blocks and tables.

---

## 🛠 Tech Stack

- **Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/) — provider clients, agent loop, RAG, streaming
- **Frontend:** [React 19](https://react.dev/) + [TypeScript](https://www.typescriptlang.org/) + [Vite 6](https://vitejs.dev/)
- **Database:** [rusqlite (SQLite)](https://github.com/rusqlite/rusqlite) — RAG vector store
- **JS Engine:** [rquickjs (QuickJS)](https://github.com/DelSkayn/rquickjs) — plugin sandbox
- **Styling:** [Tailwind CSS](https://tailwindcss.com/) + CSS variables (theming)

---

## 🚀 Quick Start / Быстрый старт

### Requirements / Требования
1. **Node.js** (LTS)
2. **Rust** & Cargo
3. Хотя бы один провайдер / at least one provider:
   - локально / local: [Ollama](https://ollama.com/) или LM Studio
   - или облачный API-ключ / or a cloud API key: OpenAI, OpenRouter, Groq, Anthropic, Gemini
4. Для RAG с локальными эмбеддингами / for RAG with local embeddings: `ollama pull nomic-embed-text`

### Installation / Установка
```bash
git clone https://github.com/kamedashe/Lumina.git
cd Lumina

npm install

npm run tauri dev
```

При первом запуске активен локальный Ollama. Облачные провайдеры добавляются в настройках (⚙): выберите пресет, вставьте ключ, нажмите «Модели» для проверки связи.

On first launch the local Ollama preset is active. Add cloud providers in Settings (⚙): pick a preset, paste your key, hit "Models" to verify the connection.

---

## 📄 Project Structure / Структура проекта

```
src-tauri/src/
  providers/        # LLM clients: ollama, openai, anthropic, gemini + SSE/NDJSON parser
  tools.rs          # Agent tool definitions & local execution
  rag.rs            # Document indexing & semantic search (SQLite)
  main.rs           # Agent loop, Tauri commands, streaming channel
src/
  components/       # ChatMessage, ProviderSettings, UI primitives
  services/         # Frontend bridges to Tauri commands
  types/            # Shared types (mirror Rust structs)
PLUGINS.md          # Plugin system documentation
```

---

## 📄 License / Лицензия

MIT License. Сделано с любовью к Open Source. 🦀💎
