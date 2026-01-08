# Lumina AI

<div align="center">
  <img src="https://via.placeholder.com/150/8B5CF6/FFFFFF?text=Lumina" alt="Lumina Logo" width="100" height="100" style="border-radius: 20px" />
  <br/>
  <br/>
  
  **A minimalist, privacy-focused local AI assistant.**
  
  Built with **Tauri**, **Rust**, and **React**. Powered by **Ollama**.

  [English](#english) | [Русский](#russian)
</div>

---

<a name="english"></a>
## 🇬🇧 English

### Overview
Lumina is a cross-platform desktop application (Windows & Linux) designed to be a sleek, efficient, and private AI companion. It runs entirely on your local machine, ensuring your data never leaves your device unless you explicitly use web features. Inspired by Spotlight and Raycast, it features a glassmorphism UI and global hotkey support.

### Features
*   **100% Local & Private:** Connects to a local Ollama instance.
*   **Minimalist UI:** Glassmorphism design with a focus on content.
*   **Global Hotkey:** Toggle the assistant instantly with `Alt + Space` (configurable).
*   **System Integration:** Ask about running processes or system stats.
*   **Web Search:** Optional privacy-friendly web search to augment AI knowledge.
*   **Model Management:** Switch between installed models (Llama 3, Mistral, etc.) or download new ones directly from the UI.
*   **Chat History:** Auto-saves your conversations locally.

### Prerequisites
Before running Lumina, ensure you have the following installed:

1.  **Node.js** (v18+)
2.  **Rust & Cargo** (latest stable)
3.  **Ollama**: Download from [ollama.com](https://ollama.com)
    *   Start the server: `ollama serve`
    *   Pull a base model: `ollama pull llama3`

### Installation

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/yourusername/lumina-ai.git
    cd lumina-ai
    ```

2.  **Install frontend dependencies:**
    ```bash
    npm install
    ```

3.  **Run in Development Mode:**
    ```bash
    npm run tauri dev
    ```
    This will compile the Rust backend and launch the application window.

### Usage

*   **Toggle Window:** Press `Alt + Space` to show/hide Lumina.
*   **Chat:** Type your query in the input bar and press Enter.
*   **System Info:** Click the "CPU" icon or ask "What processes are running?" to analyze your system.
*   **Web Search:** Toggle the "Web" button in the header to let Lumina search the internet for answers.
*   **Manage Models:** Click the Settings (gear icon) to view or download new models (e.g., type `mistral` and click Pull).

---

<a name="russian"></a>
## 🇷🇺 Русский

### Обзор
Lumina — это кроссплатформенное десктопное приложение (Windows и Linux), созданное как стильный, эффективный и приватный AI-помощник. Оно работает полностью локально, гарантируя, что ваши данные не покинут устройство без вашего ведома. Дизайн вдохновлен Spotlight и Raycast: минимализм, эффект стекла и управление горячими клавишами.

### Возможности
*   **100% Локально и Приватно:** Работает через локальный сервер Ollama.
*   **Минималистичный UI:** Дизайн в стиле Glassmorphism (матовое стекло).
*   **Глобальный хоткей:** Мгновенный вызов через `Alt + Space`.
*   **Системная интеграция:** Анализ запущенных процессов и состояния системы.
*   **Поиск в Интернете:** Опциональная функция веб-поиска для дополнения знаний ИИ.
*   **Управление моделями:** Переключение между моделями (Llama 3, Mistral и др.) и скачивание новых прямо из интерфейса.
*   **История чатов:** Автоматическое сохранение диалогов на диске.

### Требования
Перед запуском убедитесь, что у вас установлены:

1.  **Node.js** (версия 18+)
2.  **Rust & Cargo** (последняя стабильная версия)
3.  **Ollama**: Скачайте с [ollama.com](https://ollama.com)
    *   Запустите сервер: `ollama serve`
    *   Скачайте базовую модель: `ollama pull llama3`

### Установка

1.  **Клонируйте репозиторий:**
    ```bash
    git clone https://github.com/yourusername/lumina-ai.git
    cd lumina-ai
    ```

2.  **Установите зависимости:**
    ```bash
    npm install
    ```

3.  **Запуск в режиме разработки:**
    ```bash
    npm run tauri dev
    ```
    Команда скомпилирует Rust-бэкенд и откроет окно приложения.

### Использование

*   **Показать/Скрыть:** Нажмите `Alt + Space`.
*   **Чат:** Введите запрос в поле ввода и нажмите Enter.
*   **Информация о системе:** Нажмите иконку "CPU" или спросите "Какие процессы запущены?", чтобы получить анализ системы.
*   **Веб-поиск:** Включите кнопку "Web" (глобус) в шапке, чтобы Lumina могла искать информацию в интернете.
*   **Модели:** Нажмите на настройки (шестеренка), чтобы увидеть список моделей или скачать новую (например, введите `mistral` и нажмите Pull).

---

### Tech Stack
*   **Frontend:** React, TypeScript, Tailwind CSS, Framer Motion
*   **Backend:** Tauri (Rust)
*   **AI Engine:** Ollama API

### License
MIT
