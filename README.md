# Lumina 💎

**Lumina** — это современный, быстрый и конфиденциальный AI-ассистент для рабочего стола, построенный на базе **Tauri**, **Rust** и **React**. Приложение позволяет взаимодействовать с локальными языковыми моделями через Ollama, обеспечивая полную приватность ваших данных.

**Lumina** is a modern, fast, and private desktop AI assistant built with **Tauri**, **Rust**, and **React**. It allows you to interact with local LLMs via Ollama, ensuring total data privacy.

[English](#-key-features) | [Русский](#-основные-возможности)

---

## ✨ Key Features

- 🧠 **Local AI:** Seamless integration with Ollama (support for Llama 3, Mistral, and more).
- 🌐 **Web Search:** Intelligent web searching via DuckDuckGo for real-time information.
- 🎨 **Modern UI:** Elegant glassmorphism interface (Acrylic/Blur), smooth animations (Framer Motion), and dark theme.
- ⚙️ **Flexible Settings:** Control model creativity (Temperature) and switch models on the fly.
- 🔒 **Privacy First:** Your chats are stored locally and never leave your machine.
- ⚡ **High Performance:** Lightweight Rust backend and blazing fast Vite + React frontend.

---

## ✨ Основные возможности

- 🧠 **Локальный AI:** Полная интеграция с Ollama (поддержка Llama 3, Mistral и других моделей).
- 🌐 **Web Search:** Умный поиск в интернете через DuckDuckGo для получения актуальной информации.
- 🎨 **Modern UI:** Элегантный интерфейс с эффектом стекла (Acrylic/Blur), плавными анимациями (Framer Motion) и темной темой.
- ⚙️ **Гибкие настройки:** Управление "креативностью" модели (Temperature) и выбор моделей на лету.
- 🔒 **Приватность:** Ваши чаты хранятся локально и никогда не покидают ваш компьютер.
- ⚡ **Производительность:** Легковесный бэкенд на Rust и быстрый фронтенд на Vite + React.

---

## 📸 Screenshots & Demo / Скриншоты и Демо

> [Place for your GIF animation or screenshot]
*Example: `![Lumina Demo](./assets/demo.gif)`*

---

## 🚀 Quick Start / Быстрый старт

### Requirements / Требования
1. [Node.js](https://nodejs.org/) (Latest LTS).
2. [Rust](https://www.rust-lang.org/tools/install).
3. [Ollama](https://ollama.com/) (Must be running locally).

### Installation / Установка
1. Clone the repository:
   ```bash
   git clone https://github.com/your-username/lumina.git
   cd lumina
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Run in development mode:
   ```bash
   npm run tauri dev
   ```

---

## 🛠 Tech Stack / Стек технологий

- **Framework:** [Tauri v2](https://tauri.app/)
- **Frontend:** [React 19](https://react.dev/), [TypeScript](https://www.typescriptlang.org/)
- **Styling:** [Tailwind CSS](https://tailwindcss.com/), [Lucide Icons](https://lucide.dev/)
- **Animations:** [Framer Motion](https://www.framer.com/motion/)
- **Backend:** [Rust](https://www.rust-lang.org/)
- **HTTP Client:** [Reqwest](https://docs.rs/reqwest/)

---

## 📦 Build (Production) / Сборка

To create an optimized executable (.exe):

```bash
npm run tauri build
```
The file will be located in: `src-tauri/target/release/bundle/nsis/`

---

## 📄 License / Лицензия

MIT License. See [LICENSE](LICENSE) for details.

---

## 🤝 Contacts / Контакты

Developer: [Dreamota]
GitHub: [@kamedashe](https://github.com/kamedashe)

---
*Сделано с любовью и мощью Rust 🦀*
