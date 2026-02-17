import React, { useState, useEffect, useRef } from 'react';
import {
  Send, Settings, Plus, Trash2, X, History, MessageSquare, Terminal,
  Globe, Sparkles, Cpu, Code, PenTool, User, Paperclip, FileText, Blocks
} from 'lucide-react';
import './style.css';
import ReactMarkdown from 'react-markdown';
import { motion, AnimatePresence } from 'framer-motion';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

import { WindowControls, LuminaLogo, TypingIndicator } from './src/components/UI.tsx';
import { aiService } from './src/services/ai.ts';
import { agentService } from './src/services/agent.ts';
import { pluginService } from './src/services/plugins.ts';
import { Message, ChatSession } from './src/types/index.ts';

// Удалены отладочные логи, так как проблема решена расширениями

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
  const [showPlugins, setShowPlugins] = useState(false);
  const [pluginCode, setPluginCode] = useState('print("Hello from QuickJS!");\n// В QuickJS результат последнего выражения возвращается автоматически\n1 + 2;');
  const [pluginResult, setPluginResult] = useState('');
  const [temperature, setTemperature] = useState(0.7);
  const [mousePos, setMousePos] = useState({ x: 0, y: 0 });
  const [attachments, setAttachments] = useState<string[]>([]);

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const handleMouseMove = (e: React.MouseEvent) => {
    setMousePos({ x: e.clientX, y: e.clientY });
  };

  const handleAttach = async () => {
    try {
      const selected = await openDialog({
        multiple: true,
        filters: [{
          name: 'Documents',
          extensions: ['pdf', 'txt', 'md']
        }]
      });

      if (selected) {
        // Handle both single string and array of strings
        // Tauri v2 dialog returns FileResponse objects or strings depending on config, usually strings for file paths in desktop
        // Let's assume strings for now, or check types if needed.
        // In Tauri v2, it returns null | string | string[] | FileResponse | FileResponse[]
        // For simple open, it returns null | string | string[] (paths)
        const newFiles = Array.isArray(selected) ? selected : [selected];
        // Cast to string array safely
        const paths = newFiles.map((f: any) => typeof f === 'string' ? f : f.path);
        setAttachments(prev => [...prev, ...paths]);
      }
    } catch (e) {
      console.error(e);
    }
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
    const models = await aiService.getModels();
    // Фильтруем модели эмбеддингов, так как они не предназначены для чата
    const chatModels = models.filter(m => !m.toLowerCase().includes('embed'));

    setAvailableModels(chatModels.length > 0 ? chatModels : ['llama3']);
    if (chatModels.length > 0) {
      // Пытаемся сохранить выбор или ставим первую доступную текстовую модель
      setSelectedModel(prev => chatModels.includes(prev) ? prev : chatModels[0]);
    }
  };

  // --- ОТПРАВКА СООБЩЕНИЯ ---
  const handleSendMessage = async (textOverride?: string) => {
    const textToSend = textOverride || input;
    if ((!textToSend.trim() && attachments.length === 0) || isLoading) return;

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

    // Сразу создаем или обновляем сессию в истории, чтобы не потерять данные при перезагрузке
    let activeChatId = currentChatId;
    if (activeChatId === null) {
      activeChatId = Date.now();
      setCurrentChatId(activeChatId);
      const newSession: ChatSession = {
        id: activeChatId,
        title: textToSend.slice(0, 30) || "New Chat",
        date: Date.now(),
        messages: newMessages
      };
      setHistory(prev => [newSession, ...prev]);
    } else {
      setHistory(prev => prev.map(chat =>
        chat.id === activeChatId
          ? { ...chat, messages: newMessages }
          : chat
      ));
    }

    try {
      let finalPrompt = textToSend || "Analyze these files.";

      // Если есть вложения, индексируем их и сразу ищем контекст по ним
      if (attachments.length > 0) {
        setSearchStatus("Indexing & Analyzing files...");
        await aiService.processDocuments(attachments);

        // Сразу ищем контекст в только что добавленных файлах
        const ragContext = await aiService.searchDocuments(textToSend || "");
        if (ragContext) {
          finalPrompt = `Context from attached files:\n${ragContext}\n\nUser Question: ${textToSend || "Analyze the context."}`;
        }

        setAttachments([]);
      }

      // --- СИСТЕМНЫЙ ПРОМПТ ДЛЯ АГЕНТА ---
      // Добавляем инструкции по инструментам, если они еще не добавлены
      const systemPrompt = `
You are Lumina, an intelligent desktop assistant.
You have access to the following tools:
1. [Action: generate_report] - Generates a file system report of the Documents folder.
2. [Action: create_file] - Creates a new file with specified content.
3. [Action: list_files] - Lists all files and folders in a specified directory.
4. [Action: get_current_dir] - Gets the current working directory.
5. [Action: get_system_processes] - Gets the top 10 memory-consuming system processes.

To use a tool, output exactly:
- For report: @@ACTION: generate_report@@
- For creating file: @@ACTION: create_file|path/to/file.txt|file content here@@
- For listing files: @@ACTION: list_files|path/to/directory@@
- For getting current directory: @@ACTION: get_current_dir@@
- For getting system processes: @@ACTION: get_system_processes@@

Do not ask for permission, just do it if the user asks.
`;

      if (!finalPrompt.includes("You are Lumina")) {
        finalPrompt = `${systemPrompt}\n\n${finalPrompt}`;
      }



      const response = await aiService.chat(finalPrompt, selectedModel, temperature);

      // --- ПАРСИНГ ДЕЙСТВИЙ АГЕНТА ---
      let finalResponse = response;

      // Handle generate_report
      if (response.includes('@@ACTION: generate_report@@')) {
        setSearchStatus("Generating report...");
        const result = await agentService.generateWorkspaceReport();
        finalResponse = finalResponse.replace('@@ACTION: generate_report@@', `\n\n*System Action:* ${result}`);
      }

      // Handle create_file
      const createFileRegex = /@@ACTION: create_file\|(.*?)\|(.*?)@@/s;
      const createFileMatch = finalResponse.match(createFileRegex);
      if (createFileMatch) {
        const [fullMatch, path, content] = createFileMatch;
        setSearchStatus("Creating file...");
        const result = await agentService.createFile(path, content);
        finalResponse = finalResponse.replace(fullMatch, `\n\n*System Action:* ${result}`);
      }

      // Handle list_files
      const listFilesRegex = /@@ACTION: list_files\|(.*?)@@/;
      const listFilesMatch = finalResponse.match(listFilesRegex);
      if (listFilesMatch) {
        const [fullMatch, path] = listFilesMatch;
        setSearchStatus("Listing files...");
        const result = await agentService.listFiles(path);
        finalResponse = finalResponse.replace(fullMatch, `\n\n*System Action (List Files):*\n\`\`\`\n${result}\n\`\`\``);
      }

      // Handle get_current_dir
      if (response.includes('@@ACTION: get_current_dir@@')) {
        setSearchStatus("Getting current directory...");
        const result = await agentService.getCurrentDir();
        finalResponse = finalResponse.replace('@@ACTION: get_current_dir@@', `\n\n*System Action (Current Dir):* ${result}`);
      }

      // Handle get_system_processes
      if (response.includes('@@ACTION: get_system_processes@@')) {
        setSearchStatus("Getting system processes...");
        const result = await agentService.getSystemProcesses();
        finalResponse = finalResponse.replace('@@ACTION: get_system_processes@@', `\n\n*System Action (Processes):*\n\`\`\`\n${result.join('\n')}\n\`\`\``);
      }

      const assistantMsg: Message = { role: 'assistant', content: finalResponse };
      const updatedMessages = [...newMessages, assistantMsg];
      setMessages(updatedMessages);

      // --- ЛОГИКА СОХРАНЕНИЯ ЧАТА ---
      // Теперь мы просто обновляем существующую сессию, так как она была создана в начале функции
      setHistory(prev => prev.map(chat =>
        chat.id === activeChatId
          ? { ...chat, messages: updatedMessages }
          : chat
      ));

      // Если это был новый чат, пробуем улучшить заголовок
      if (currentChatId === null) {
        aiService.generateTitle(textToSend, selectedModel).then((smartTitle) => {
          setHistory(prev => prev.map(chat =>
            chat.id === activeChatId
              ? { ...chat, title: smartTitle }
              : chat
          ));
        });
      }

    } catch (e) {
      console.error("Chat error details:", e);
      setMessages(prev => [...prev, { role: 'assistant', content: `Error: ${e}` }]);
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

          <button onClick={() => setShowPlugins(true)} className="w-8 h-8 flex items-center justify-center rounded-lg hover:bg-white/10 text-gray-400 hover:text-white transition-colors">
            <Blocks size={18} />
          </button>
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
                <div className="relative">
                  {attachments.length > 0 && (
                    <div className="absolute bottom-full left-0 mb-2 w-64 bg-[#1e1e24] border border-white/10 rounded-lg p-2 shadow-xl">
                      <div className="text-[10px] font-bold text-gray-500 uppercase mb-1 px-1">Attached Files</div>
                      {attachments.map((file, i) => (
                        <div key={i} className="flex items-center gap-2 p-1.5 hover:bg-white/5 rounded text-xs text-gray-300 truncate group">
                          <FileText size={12} className="shrink-0 text-purple-400" />
                          <span className="truncate flex-1">{file.split(/[\\/]/).pop()}</span>
                          <button
                            onClick={() => setAttachments(prev => prev.filter((_, idx) => idx !== i))}
                            className="opacity-0 group-hover:opacity-100 hover:text-red-400 transition-opacity"
                          >
                            <X size={12} />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                  <button
                    onClick={handleAttach}
                    className={`p-2.5 rounded-lg transition-all ${attachments.length > 0 ? 'text-purple-400 bg-purple-500/10' : 'text-gray-500 hover:text-gray-300'}`}
                    title="Attach files (PDF/TXT)"
                  >
                    <Paperclip size={18} />
                  </button>
                </div>
                <input
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleSendMessage()}
                  disabled={isLoading}
                  placeholder={isLoading ? "Lumina is busy..." : "Ask Lumina anything..."}
                  className={`flex-1 bg-transparent px-4 py-3 outline-none text-sm transition-colors ${isLoading ? 'text-gray-600 cursor-not-allowed' : 'text-gray-200 placeholder-gray-600'
                    }`}
                />
                <button
                  onClick={() => handleSendMessage()}
                  disabled={isLoading || (!input.trim() && attachments.length === 0)}
                  className={`p-2.5 rounded-lg transition-all ${input.trim() || attachments.length > 0
                    ? 'bg-purple-600 text-white shadow-[0_0_15px_rgba(147,51,234,0.5)]'
                    : 'bg-white/5 text-gray-500 hover:bg-white/10'
                    }`}
                >
                  <Send size={18} />
                </button>
              </div>
            </div>
            <p className="text-[10px] text-center mt-3 text-gray-600 uppercase tracking-widest font-medium select-none">
              Private & Local AI
            </p>
          </div>
        </main>
      </div>

      {/* Plugin Modal */}
      <AnimatePresence>
        {showPlugins && (
          <motion.div
            initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
            className="absolute inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center"
            onClick={() => setShowPlugins(false)}
          >
            <motion.div
              initial={{ scale: 0.95, y: 10 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.95, y: 10 }}
              className="bg-[#121212] border border-white/10 rounded-2xl p-6 w-[600px] h-[500px] shadow-2xl flex flex-col"
              onClick={e => e.stopPropagation()}
            >
              <div className="flex justify-between items-center mb-4">
                <h2 className="text-xl font-bold text-white flex items-center gap-2"><Blocks size={20} className="text-purple-500" /> Plugins (QuickJS)</h2>
                <button onClick={() => setShowPlugins(false)} className="text-gray-500 hover:text-white"><X size={20} /></button>
              </div>

              <div className="flex-1 flex flex-col gap-4">
                <textarea
                  value={pluginCode}
                  onChange={(e) => setPluginCode(e.target.value)}
                  className="flex-1 bg-[#0a0a0a] border border-white/10 rounded-xl p-4 text-sm font-mono text-green-400 resize-none outline-none focus:border-purple-500/50 transition-colors"
                  placeholder="// Enter JavaScript code here..."
                />

                {pluginResult && (
                  <div className="bg-white/5 rounded-xl p-4 border border-white/5">
                    <div className="text-[10px] font-bold text-gray-500 uppercase mb-1">Result</div>
                    <pre className="text-sm text-gray-300 font-mono whitespace-pre-wrap">{pluginResult}</pre>
                  </div>
                )}

                <button
                  onClick={async () => {
                    const res = await pluginService.runPlugin(pluginCode);
                    setPluginResult(res);
                  }}
                  className="w-full py-3 bg-gradient-to-r from-purple-600 to-indigo-600 hover:from-purple-500 hover:to-indigo-500 text-white font-bold rounded-xl transition-all shadow-lg"
                >
                  Run Plugin
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>

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