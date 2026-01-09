import React, { useState, useEffect, useRef } from 'react';
import {
  Send, Settings, Plus, Trash2, X, Minus, Square, History, MessageSquare, Terminal,
  Globe, Sparkles, Cpu, Code, PenTool, User
} from 'lucide-react';
import './style.css';
import ReactMarkdown from 'react-markdown';
import { motion, AnimatePresence } from 'framer-motion';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';

// --- КНОПКИ ОКНА ---
const WindowControls = () => {
  const appWindow = getCurrentWindow();
  return (
    <div className="flex items-center gap-2 z-50 ml-2">
      <button onClick={() => appWindow.minimize()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors group">
        <Minus size={16} />
      </button>
      <button onClick={() => appWindow.toggleMaximize()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors group">
        <Square size={14} />
      </button>
      <button onClick={() => appWindow.close()} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-red-500/20 text-gray-400 hover:text-red-400 transition-colors">
        <X size={18} />
      </button>
    </div>
  );
};

// --- ЛОГОТИП ---
const LuminaLogo: React.FC<{ className?: string }> = ({ className = "w-8 h-8" }) => (
  <div className={`relative flex items-center justify-center ${className}`}>
    <div className="absolute inset-0 bg-purple-600 blur-[25px] opacity-60 animate-pulse"></div>
    <svg viewBox="0 0 100 100" className="relative w-full h-full z-10 drop-shadow-[0_0_15px_rgba(168,85,247,0.5)]">
      <path d="M50 10 L90 50 L50 90 L10 50 Z" stroke="white" strokeWidth="6" fill="transparent" />
      <circle cx="50" cy="50" r="6" fill="#a855f7" className="animate-pulse" />
    </svg>
  </div>
);

// --- ИНДИКАТОР ПЕЧАТИ ---
const TypingIndicator = () => (
  <div className="flex items-center gap-3 p-4 bg-[#1e1e24]/90 rounded-2xl w-fit border border-white/5 shadow-xl">
    <div className="flex gap-1">
      <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0 }} />
      <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0.2 }} />
      <motion.div className="w-2 h-2 bg-purple-500 rounded-full" animate={{ y: [0, -5, 0] }} transition={{ duration: 0.6, repeat: Infinity, delay: 0.4 }} />
    </div>
    <span className="text-xs font-medium text-gray-400 animate-pulse">Lumina thinking...</span>
  </div>
);

// --- ТИПЫ ---
type Message = { role: 'user' | 'assistant'; content: string };
type ChatSession = { id: number; title: string; date: number; messages: Message[] };

// --- ОСНОВНОЙ КОМПОНЕНТ ---
const App: React.FC = () => {
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [history, setHistory] = useState<ChatSession[]>([]);
  const [searchStatus, setSearchStatus] = useState<string | null>(null);

  // ID текущего чата. Если null - это новый черновик.
  const [currentChatId, setCurrentChatId] = useState<number | null>(null);

  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [selectedModel, setSelectedModel] = useState('llama3');
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);
  const [showSettings, setShowSettings] = useState(false);
  const [isWebEnabled, setIsWebEnabled] = useState(false);
  const [temperature, setTemperature] = useState(0.7);
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const handleMouseMove = (e: React.MouseEvent) => {
    setMousePos({ x: e.clientX, y: e.clientY });
  };

  useEffect(() => {
    fetchModels();
    const saved = localStorage.getItem('lumina_history');
    if (saved) setHistory(JSON.parse(saved));
  }, []);

  // Сохраняем историю при любом изменении
  useEffect(() => {
    if (history.length > 0) {
      localStorage.setItem('lumina_history', JSON.stringify(history));
    }
  }, [history]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, isLoading]);

  const fetchModels = async () => {
    try {
      const models = await invoke<string[]>('get_ollama_models');
      setAvailableModels(models || ['llama3']);
      if (models && models.length > 0) setSelectedModel(models[0]);
    } catch (e) {
      setAvailableModels(['llama3']);
    }
  };

  // --- ГЕНЕРАЦИЯ УМНОГО ЗАГОЛОВКА ---
  const generateChatTitle = async (firstMessage: string) => {
    try {
      // Просим модель придумать заголовок
      const title = await invoke<string>('chat_with_ollama', {
        model: selectedModel,
        prompt: `Generate a very short title (max 4 words) for a chat that starts with this message: "${firstMessage}". Do not use quotes. Just the title.`,
        temperature: 0.3
      });
      return title.trim();
    } catch (e) {
      // Если ошибка, берем первые 20 символов
      return firstMessage.slice(0, 20) + "...";
    }
  };

  // --- ОТПРАВКА СООБЩЕНИЯ ---
  const handleSendMessage = async (textOverride?: string) => {
    const textToSend = textOverride || input;
    if (!textToSend.trim() || isLoading) return;

    const currentDate = new Date().toLocaleString('ru-RU', {
      year: 'numeric',
      month: 'long',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });

    setInput('');
    setIsLoading(true);

    const userMsg: Message = { role: 'user', content: textToSend };
    // Обновляем UI мгновенно
    const newMessages = [...messages, userMsg];
    setMessages(newMessages);

    try {
      let finalPrompt = textToSend;

      if (isWebEnabled) {
        // Пропускаем поиск для очень коротких или простых фраз
        const skipSearchWords = ['привет', 'hi', 'hello', 'как дела', 'кто ты', 'что ты умеешь'];
        const shouldSkipSearch = skipSearchWords.some(word => textToSend.toLowerCase().trim() === word);

        if (!shouldSkipSearch) {
          setSearchStatus("Optimizing query...");
          // 1. Генерируем поисковый запрос
          const searchQuery = await invoke<string>('chat_with_ollama', {
            model: selectedModel,
            prompt: `Create a short English search query for: "${textToSend}". Return ONLY the query.`,
            temperature: 0.1
          });

          const cleanQuery = searchQuery.trim().replace(/^"|"$/g, '');
          setSearchStatus(`Searching: ${cleanQuery}...`);

          // 2. Выполняем поиск
          const searchContext = await invoke<string>('web_search', { query: cleanQuery });
          setSearchStatus("Analyzing...");

          // 3. Формируем финальный промпт с контекстом
          finalPrompt = `You are Lumina, an AI with web access.
Current Date: ${currentDate}
Knowledge Cutoff: 2023

Search Results:
${searchContext}

User: ${textToSend}

Instructions:
1. Use ONLY the search results if they contain the answer.
2. If the search results are empty or irrelevant, use your internal knowledge.
3. If you use search results, mention that you found this information online.
4. ALWAYS reply in Russian.
5. Today is ${currentDate}.

Answer:`;
        }
      }

      const response = await invoke<string>('chat_with_ollama', {
        model: selectedModel,
        prompt: finalPrompt,
        temperature: temperature
      });

      const assistantMsg: Message = { role: 'assistant', content: response };
      const updatedMessages = [...newMessages, assistantMsg];
      setMessages(updatedMessages);

      // --- ЛОГИКА СОХРАНЕНИЯ ЧАТА ---
      if (currentChatId === null) {
        // ЭТО НОВЫЙ ЧАТ: Генерируем ID и заголовок
        const newId = Date.now();
        setCurrentChatId(newId);

        // Генерируем заголовок асинхронно (чтобы не фризить UI)
        generateChatTitle(textToSend).then((smartTitle) => {
          setHistory(prev => [{
            id: newId,
            title: smartTitle,
            date: Date.now(),
            messages: updatedMessages
          }, ...prev]);
        });
      } else {
        // ЭТО СТАРЫЙ ЧАТ: Просто обновляем историю
        setHistory(prev => prev.map(chat =>
          chat.id === currentChatId
            ? { ...chat, messages: updatedMessages }
            : chat
        ));
      }

    } catch (e) {
      setMessages(prev => [...prev, { role: 'assistant', content: "Error: Ollama not responding." }]);
    } finally {
      setIsLoading(false);
      setSearchStatus(null);
    }
  };

  // --- НОВЫЙ ЧАТ (Просто сброс UI, без сохранения пустых) ---
  const startNewChat = () => {
    setMessages([]);
    setCurrentChatId(null); // Сбрасываем ID, переходим в режим "Черновик"
  };

  // --- ВЫБОР ЧАТА ИЗ ИСТОРИИ ---
  const selectChat = (chat: ChatSession) => {
    setCurrentChatId(chat.id);
    setMessages(chat.messages);
  };

  const deleteChat = (e: React.MouseEvent, id: number) => {
    e.stopPropagation();
    const newHistory = history.filter(h => h.id !== id);
    setHistory(newHistory);
    // Если удалили текущий открытый чат - сбрасываем
    if (currentChatId === id) {
      startNewChat();
    }
  };

  return (
    <div
      onMouseMove={handleMouseMove}
      className="h-screen w-screen flex flex-col bg-[#050505] text-white font-sans overflow-hidden relative"
    >
      <div
        className="pointer-events-none fixed inset-0 z-0 opacity-20"
        style={{ background: `radial-gradient(600px circle at ${mousePos.x}px ${mousePos.y}px, rgba(120, 50, 255, 0.15), transparent 40%)` }}
      />

      {/* HEADER */}
      <header className="h-14 border-b border-white/5 bg-black/40 flex items-center justify-between px-5 shrink-0 backdrop-blur-md z-20" data-tauri-drag-region>
        <div className="flex items-center gap-3" data-tauri-drag-region>
          <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className="text-gray-400 hover:text-white transition-colors">
            <History size={20} />
          </button>
          <LuminaLogo className="w-7 h-7" />
          <span className="font-bold text-xl tracking-wide bg-clip-text text-transparent bg-gradient-to-r from-white to-gray-400 select-none" data-tauri-drag-region>
            Lumina
          </span>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={() => setIsWebEnabled(!isWebEnabled)}
            className={`h-8 px-3 rounded-full flex items-center gap-2 text-xs font-bold border transition-all ${isWebEnabled
              ? 'bg-blue-600/20 border-blue-500/50 text-blue-400 shadow-[0_0_10px_rgba(59,130,246,0.3)]'
              : 'bg-white/5 border-white/5 text-gray-500 hover:bg-white/10'
              }`}
          >
            <Globe size={14} />
            WEB
          </button>

          <div className="px-3 py-1 bg-white/5 border border-white/5 rounded-full flex items-center gap-2">
            <div className="w-1.5 h-1.5 rounded-full bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.8)] animate-pulse"></div>
            <select
              value={selectedModel}
              onChange={(e) => setSelectedModel(e.target.value)}
              className="bg-transparent text-xs font-medium outline-none text-gray-300 hover:text-white cursor-pointer uppercase tracking-wider w-32 truncate"
            >
              {availableModels.map(m => <option key={m} value={m} className="bg-[#0F0F13]">{m}</option>)}
            </select>
          </div>

          <button onClick={() => setShowSettings(true)} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors">
            <Settings size={18} />
          </button>
          <div className="h-4 w-[1px] bg-white/10 mx-1"></div>
          <WindowControls />
        </div>
      </header>

      {/* CONTENT */}
      <div className="flex-1 flex overflow-hidden relative z-10">

        {/* SIDEBAR */}
        <AnimatePresence>
          {isSidebarOpen && (
            <motion.aside
              initial={{ width: 0, opacity: 0 }}
              animate={{ width: 260, opacity: 1 }}
              exit={{ width: 0, opacity: 0 }}
              className="border-r border-white/5 bg-black/60 backdrop-blur-xl overflow-hidden flex flex-col"
            >
              <div className="p-4">
                <button
                  onClick={startNewChat}
                  className="w-full py-3 bg-gradient-to-r from-purple-900/40 to-blue-900/40 hover:from-purple-800/50 hover:to-blue-800/50 border border-white/10 rounded-xl text-sm font-medium text-white transition-all flex items-center justify-center gap-2 shadow-lg group"
                >
                  <Plus size={16} className="group-hover:rotate-90 transition-transform" /> New Chat
                </button>
              </div>

              <div className="flex-1 overflow-y-auto px-2 custom-scrollbar">
                <div className="text-[10px] font-bold text-gray-600 uppercase tracking-widest mb-2 pl-4 mt-2">Your Chats</div>
                {history.map((h) => (
                  <div key={h.id} className="group relative mb-1">
                    <button
                      onClick={() => selectChat(h)}
                      className={`w-full text-left p-3 pr-8 rounded-lg text-xs truncate flex items-center gap-3 transition-all
                        ${currentChatId === h.id
                          ? 'bg-purple-600/20 text-white border border-purple-500/20 shadow-md'
                          : 'text-gray-400 hover:bg-white/5 hover:text-gray-200 border border-transparent'
                        }`}
                    >
                      <MessageSquare size={14} className={currentChatId === h.id ? 'text-purple-400' : 'opacity-50'} />
                      {h.title}
                    </button>
                    <button
                      onClick={(e) => deleteChat(e, h.id)}
                      className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 p-1.5 hover:bg-red-500/20 text-gray-500 hover:text-red-400 rounded transition-all"
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                ))}
              </div>

              <div className="p-4 border-t border-white/5">
                <button onClick={() => { localStorage.clear(); setHistory([]); startNewChat(); }} className="w-full py-2 flex items-center justify-center gap-2 text-xs text-gray-500 hover:text-red-400 transition-colors">
                  <Trash2 size={14} /> Clear All
                </button>
              </div>
            </motion.aside>
          )}
        </AnimatePresence>

        {/* CHAT AREA */}
        <main className="flex-1 flex flex-col relative">
          <div className="flex-1 overflow-y-auto p-6 space-y-6 custom-scrollbar">
            {messages.length === 0 ? (
              // START SCREEN
              <div className="h-full flex flex-col items-center justify-center select-none animate-in fade-in duration-700">
                <div className="relative group mb-8">
                  <div className="absolute inset-0 bg-purple-500 blur-[50px] opacity-20 group-hover:opacity-40 transition-opacity duration-1000"></div>
                  <LuminaLogo className="w-24 h-24 relative z-10" />
                </div>

                <h1 className="text-3xl font-bold text-white mb-8 tracking-tight drop-shadow-lg">How can I help you?</h1>

                <div className="grid grid-cols-2 gap-3 max-w-2xl w-full px-4">
                  <button onClick={() => handleSendMessage("Analyze my system processes")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/20 rounded-2xl text-left transition-all group">
                    <Cpu className="w-5 h-5 text-purple-400 mb-2 group-hover:scale-110 transition-transform" />
                    <div className="text-sm font-medium text-gray-200">System Check</div>
                  </button>
                  <button onClick={() => handleSendMessage("Write a Python script to parse JSON")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/20 rounded-2xl text-left transition-all group">
                    <Code className="w-5 h-5 text-green-400 mb-2 group-hover:scale-110 transition-transform" />
                    <div className="text-sm font-medium text-gray-200">Coding Help</div>
                  </button>
                  <button onClick={() => handleSendMessage("Explain Quantum Computing")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/20 rounded-2xl text-left transition-all group">
                    <Sparkles className="w-5 h-5 text-blue-400 mb-2 group-hover:scale-110 transition-transform" />
                    <div className="text-sm font-medium text-gray-200">Explain It</div>
                  </button>
                  <button onClick={() => handleSendMessage("Draft an email to my team")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/20 rounded-2xl text-left transition-all group">
                    <PenTool className="w-5 h-5 text-orange-400 mb-2 group-hover:scale-110 transition-transform" />
                    <div className="text-sm font-medium text-gray-200">Writing Assistant</div>
                  </button>
                </div>
              </div>
            ) : (
              // MESSAGES
              <>
                {messages.map((m, i) => (
                  <div key={i} className={`flex gap-3 ${m.role === 'user' ? 'flex-row-reverse' : 'flex-row'}`}>
                    <div className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 shadow-lg ${m.role === 'user'
                      ? 'bg-gradient-to-br from-purple-600 to-indigo-600'
                      : 'bg-[#1e1e24] border border-white/10'
                      }`}>
                      {m.role === 'user' ? <User size={16} className="text-white" /> : <LuminaLogo className="w-5 h-5" />}
                    </div>
                    <div className={`max-w-[80%] p-4 rounded-2xl text-sm leading-relaxed shadow-lg backdrop-blur-sm
                      ${m.role === 'user'
                        ? 'bg-gradient-to-br from-purple-600/90 to-indigo-600/90 text-white border border-white/10 rounded-tr-none'
                        : 'bg-[#1e1e24]/90 text-gray-200 border border-white/5 rounded-tl-none'}`}
                    >
                      <div className="text-[10px] font-bold uppercase tracking-wider mb-1 opacity-50">
                        {m.role === 'user' ? 'You' : 'Lumina'}
                      </div>
                      <ReactMarkdown>{m.content}</ReactMarkdown>
                    </div>
                  </div>
                ))}
                {isLoading && (
                  <div className="flex flex-col gap-3">
                    {searchStatus && (
                      <div className="flex justify-start items-center gap-2 ml-11">
                        <div className="w-4 h-4 border-2 border-blue-500 border-t-transparent rounded-full animate-spin"></div>
                        <span className="text-[10px] font-bold text-blue-400 uppercase tracking-widest">{searchStatus}</span>
                      </div>
                    )}
                    <div className="flex justify-start gap-3">
                      <div className="w-8 h-8 rounded-full bg-[#1e1e24] border border-white/10 flex items-center justify-center shrink-0 shadow-lg">
                        <LuminaLogo className="w-5 h-5" />
                      </div>
                      <TypingIndicator />
                    </div>
                  </div>
                )}
              </>
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* INPUT */}
          <div className="p-6 pt-2 z-20">
            <div className="max-w-4xl mx-auto relative group">
              <div className="absolute -inset-[1px] bg-gradient-to-r from-purple-500 to-blue-500 rounded-xl opacity-0 group-focus-within:opacity-40 transition-opacity duration-500 blur-[8px]"></div>
              <div className="relative flex items-center bg-[#0a0a0a]/90 backdrop-blur-xl border border-white/10 rounded-xl p-2 shadow-2xl transition-all">
                <input
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleSendMessage()}
                  disabled={isLoading}
                  placeholder={isLoading ? "Lumina is busy..." : (isWebEnabled ? "Search the web..." : "Ask Lumina anything...")}
                  className={`flex-1 bg-transparent px-4 py-3 outline-none text-sm transition-colors ${isLoading ? 'text-gray-600 cursor-not-allowed' : 'text-gray-200 placeholder-gray-600'
                    }`}
                />
                <button
                  onClick={() => handleSendMessage()}
                  disabled={isLoading || !input.trim()}
                  className={`p-2.5 rounded-lg transition-all ${input.trim()
                    ? 'bg-purple-600 text-white shadow-[0_0_15px_rgba(147,51,234,0.5)]'
                    : 'bg-white/5 text-gray-500 hover:bg-white/10'
                    }`}
                >
                  <Send size={18} />
                </button>
              </div>
            </div>
            <p className="text-[10px] text-center mt-3 text-gray-600 uppercase tracking-widest font-medium select-none">
              {isWebEnabled ? 'Web Mode Active' : 'Private & Local AI'}
            </p>
          </div>
        </main>
      </div>

      {/* Settings Modal (Keep existing) */}
      <AnimatePresence>
        {showSettings && (
          <motion.div
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            className="absolute inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center"
            onClick={() => setShowSettings(false)}
          >
            <motion.div
              initial={{ scale: 0.95, y: 10 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, y: 10 }}
              className="bg-[#121212] border border-white/10 rounded-2xl p-6 w-[450px] shadow-2xl"
              onClick={e => e.stopPropagation()}
            >
              <div className="flex justify-between items-center mb-6">
                <h2 className="text-xl font-bold text-white">Settings</h2>
                <button onClick={() => setShowSettings(false)} className="text-gray-500 hover:text-white"><X size={20} /></button>
              </div>
              <div className="space-y-6">
                <div className="p-4 bg-white/5 rounded-xl border border-white/5 flex items-center gap-4">
                  <div className="w-10 h-10 rounded-full bg-green-500/20 flex items-center justify-center text-green-500"><Terminal size={20} /></div>
                  <div><div className="text-sm font-medium text-white">Ollama Connection</div><div className="text-xs text-green-400">● Active</div></div>
                </div>
                <div className="space-y-4">
                  <div>
                    <div className="flex justify-between text-xs text-gray-400 mb-2">
                      <span>Temperature</span>
                      <span>{temperature}</span>
                    </div>
                    <input
                      type="range"
                      min="0"
                      max="1"
                      step="0.1"
                      value={temperature}
                      onChange={(e) => setTemperature(parseFloat(e.target.value))}
                      className="w-full h-1 bg-white/10 rounded-full appearance-none cursor-pointer accent-purple-500"
                    />
                  </div>
                </div>
              </div>
              <button onClick={() => setShowSettings(false)} className="mt-6 w-full py-2 bg-white text-black hover:bg-gray-200 rounded-lg font-bold transition-colors">Save Changes</button>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

export default App;