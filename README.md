# Lumina 💎 — Agentic OS Assistant

**Lumina** — это продвинутый агентный AI-ассистент для рабочего стола, объединяющий мощь локальных языковых моделей с прямым доступом к системе. В отличие от обычных чат-ботов, Lumina может взаимодействовать с вашими файлами, анализировать процессы и выполнять код в безопасной песочнице.

**Lumina** is an advanced Agentic OS Assistant that bridges the gap between local LLMs and system-level operations. Unlike standard chatbots, Lumina can interact with your file system, analyze system processes, and execute custom plugins in a secure sandbox.

[English](#-key-specialization) | [Русский](#-специализация)

---

## 🚀 Key Specialization / Специализация

### 🤖 Agentic Workflows / Агентные функции
Lumina может выполнять сложные задачи, используя встроенные инструменты:
- **Составление отчетов**: Анализ структуры проекта.
- **Управление файлами**: Чтение и создание файлов напрямую.
- **Системная осведомленность**: Просмотр запущенных процессов и системной информации.

Lumina performs complex tasks via built-in system tools:
- **Workspace Analytics**: Recursive reporting of project structures.
- **File Orchestration**: Direct reading and creation of workspace files.
- **System Awareness**: Monitoring active processes and environment state.

### 📚 Local RAG (Knowledge Base) / Локальный RAG
Поддержка **Retrieval-Augmented Generation**:
- Индексация PDF, TXT, MD, JSON и исходного кода (Rust, TS, JS).
- Поиск по смыслу в локальной векторной базе данных (SQLite + Эмбеддинги).
- Ответы на основе ваших документов без отправки данных в облако.

Full **Retrieval-Augmented Generation** support:
- Indexing PDF, TXT, MD, JSON, and source code (Rust, TS, JS).
- Semantic search using a local vector store (SQLite + Embeddings).
- Context-aware answers based exclusively on your local data.

### 🔌 Plugin System / Система плагинов
Безопасная песочница **QuickJS**:
- Выполнение произвольного JavaScript кода внутри Rust-окружения.
- Автоматизация рутинных задач через кастомные скрипты ("Blocks").

Secure **QuickJS** sandbox:
- Execution of arbitrary JavaScript code isolated within Rust.
- Automation of routine tasks via custom logic "Blocks".

---

## ✨ Features / Возможности

- 🧠 **Ollama Powered:** Использование любых локальных моделей (Llama 3, Mistral, Gemma).
- 🎨 **Premium UI:** Интерфейс с эффектом **Acrylic/Mica**, глубоким размытием и плавными анимациями.
- 🔒 **Privacy First:** Полная автономность — данные не покидают компьютер.
- ⚡ **Turbo Performance:** Минимальное потребление ресурсов благодаря Rust-бэкенду.
- 🌐 **Web Integration:** Интеллектуальный поиск информации в сети.

---

## 🛠 Tech Stack / Стек технологий

- **Backend:** [Rust](https://www.rust-lang.org/) + [Tauri v2](https://tauri.app/)
- **Frontend:** [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/), [Vite 6](https://vitejs.dev/)
- **Database:** [Rusqlite (SQLite)](https://github.com/rusqlite/rusqlite) for RAG context.
- **JS Engine:** [rquickjs (QuickJS)](https://github.com/DelSkayn/rquickjs) for plugins.
- **Styling:** [Tailwind CSS](https://tailwindcss.com/), [Framer Motion](https://www.framer.com/motion/)
- **AI Core:** [Ollama](https://ollama.com/) (REST API)

---

## 🚀 Quick Start / Быстрый старт

### Requirements / Требования
1. **Node.js** (LTS)
2. **Rust** & Cargo
3. **Ollama** (запущенный локально)
4. Модель для эмбеддингов: `ollama pull nomic-embed-text`

### Installation / Установка
```bash
# Clone the repo
git clone https://github.com/kamedashe/Lumina.git
cd Lumina

# Install dependencies
npm install

# Run in dev mode
npm run tauri dev
```

---

## 📄 Project Structure / Структура проекта

- `src/` — React frontend (Home of the Agent UI).
- `src-tauri/` — Rust backend (Core logic, DB, Plugin system, System API).
- `src/services/` — Logic for AI, Agents, and Plugins.
- `PLUGINS.md` — Documentation for the plugin system.

---

## 📄 License / Лицензия

MIT License. Сделано с любовью к Open Source и локальному AI. 🦀💎
