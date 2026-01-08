import React, { useState, useEffect, useRef } from 'react';
import { 
    Send, Cpu, Loader2, Bot, Sparkles,
    Settings, Globe, History, ChevronRight, MessageSquare, Plus, Trash2, User, 
    Terminal, Command
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { motion, AnimatePresence } from 'framer-motion';
import { Message, ChatSession } from './types';
import { SYSTEM_PROMPT } from './constants';

// --- Safe Invoke Handling ---
const invoke = async <T,>(cmd: string, args?: any): Promise<T> => {
  if (window.__TAURI__) {
    try {
      return await window.__TAURI__.invoke(cmd, args);
    } catch (err) {
      console.warn(`Tauri invoke failed for ${cmd}:`, err);
      // Fall through to mock data on failure
    }
  }

  // --- Mock Data for Development/Preview ---
  await new Promise(r => setTimeout(r, 800)); // Simulate network latency
  
  if (cmd === 'chat_with_ollama') {
    const userContent = args?.messages?.[args.messages.length - 1]?.content || "";
    return `**Lumina (Mock):** I am currently running in **UI Preview Mode**. 
    
To connect to the real local LLM, ensure:
1. **Ollama** is running locally.
2. You have pulled the model (e.g., \`ollama pull llama3\`).
3. The Tauri backend is compiled and running.

Your input was: _"${userContent}"_` as any;
  }
  if (cmd === 'get_ollama_models') return ['llama3', 'mistral', 'neural-chat', 'phi3'] as any;
  if (cmd === 'get_system_processes') return ['Code.exe (Memory: 450MB)', 'Chrome.exe (Memory: 1.2GB)', 'Ollama (GPU: 4GB)', 'Lumina (Memory: 80MB)'] as any;
  
  return null as any;
};

// --- Components ---

const LuminaLogo: React.FC<{ className?: string, withText?: boolean }> = ({ className = "w-8 h-8", withText = false }) => (
  <div className={`flex items-center gap-3 ${withText ? '' : 'justify-center'}`}>
    <div className="relative flex items-center justify-center">
      <div className="absolute inset-0 bg-purple-500 blur-xl opacity-40 animate-pulse-slow"></div>
      <svg className={`relative z-10 ${className}`} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="logo_grad" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#C084FC" />
            <stop offset="100%" stopColor="#6366F1" />
          </linearGradient>
        </defs>
        <path 
          d="M50 15 L85 50 L50 85 L15 50 Z" 
          stroke="url(#logo_grad)" 
          strokeWidth="8"
          strokeLinecap="round"
          strokeLinejoin="round"
          fill="rgba(167, 139, 250, 0.1)"
        />
        <circle cx="50" cy="50" r="6" fill="#FFF" className="animate-pulse" />
      </svg>
    </div>
    {withText && (
      <span className="text-2xl font-bold tracking-tight text-white drop-shadow-md">
        Lumina
      </span>
    )}
  </div>
);

const App: React.FC = () => {
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [currentSessionId, setCurrentSessionId] = useState<string>(Date.now().toString());
  const [history, setHistory] = useState<ChatSession[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>('llama3');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [isWebSearchEnabled, setIsWebSearchEnabled] = useState(false); 

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Initialize
  useEffect(() => {
    const saved = localStorage.getItem('lumina_history');
    if (saved) {
      try {
        setHistory(JSON.parse(saved));
      } catch (e) {
        console.error("Failed to parse history", e);
      }
    }
    fetchModels();
  }, []);

  // Auto-scroll and save
  useEffect(() => {
    if (messages.length > 0) {
      scrollToBottom();
      saveHistory();
    }
  }, [messages]);

  const saveHistory = () => {
    if (messages.length === 0) return;
    setHistory(prev => {
        const existingIndex = prev.findIndex(s => s.id === currentSessionId);
        const firstMsg = messages.find(m => m.role === 'user')?.content || 'New Chat';
        const title = firstMsg.length > 30 ? firstMsg.substring(0, 30) + '...' : firstMsg;
        
        const updatedSession = { id: currentSessionId, title, date: Date.now(), messages };
        
        let newHistory;
        if (existingIndex >= 0) {
            newHistory = [...prev];
            newHistory[existingIndex] = updatedSession;
        } else {
            newHistory = [updatedSession, ...prev];
        }
        localStorage.setItem('lumina_history', JSON.stringify(newHistory));
        return newHistory;
    });
  };

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  const fetchModels = async () => {
    try {
      const models = await invoke<string[]>('get_ollama_models');
      if (models && models.length > 0) {
        setAvailableModels(models);
        if (!models.includes(selectedModel)) setSelectedModel(models[0]);
      } else {
        setAvailableModels(['llama3', 'mistral']); // Fallback
      }
    } catch (e) {
      setAvailableModels(['llama3', 'mistral']); // Fallback
    }
  };

  const handleSendMessage = async () => {
    if (!input.trim() || isLoading) return;
    const userMsg: Message = { role: 'user', content: input };
    setMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);

    try {
        const response = await invoke<string>('chat_with_ollama', {
          model: selectedModel,
          messages: [{role: 'system', content: SYSTEM_PROMPT}, ...messages, userMsg]
        });
        
        setMessages(prev => [...prev, { role: 'assistant', content: response }]);
    } catch (e: any) {
      setMessages(prev => [...prev, { role: 'assistant', content: "Error: Could not connect to the AI engine. Is Ollama running?" }]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleGetProcesses = async () => {
    setIsLoading(true);
    const sysMsg: Message = { role: 'user', content: "Analyze system processes" };
    setMessages(prev => [...prev, sysMsg]);
    
    try {
        const procs = await invoke<string[]>('get_system_processes');
        const context = `Top active processes:\n${procs.join('\n')}\n\nAnalyze these for high resource usage.`;
        const response = await invoke<string>('chat_with_ollama', {
            model: selectedModel,
            messages: [{role: 'system', content: SYSTEM_PROMPT}, { role: 'user', content: context }]
        });
        setMessages(prev => [...prev, { role: 'assistant', content: response }]);
    } catch (e) {
        setMessages(prev => [...prev, { role: 'assistant', content: "Unable to access process list." }]);
    } finally {
        setIsLoading(false);
    }
  };

  const startNewSession = () => {
    setMessages([]);
    setCurrentSessionId(Date.now().toString());
    setTimeout(() => inputRef.current?.focus(), 100);
  };

  return (
    <div className="h-screen w-screen flex items-center justify-center font-sans overflow-hidden text-white selection:bg-purple-500/30">
      <div className="w-full h-full md:w-[900px] md:h-[680px] glass-panel rounded-2xl flex shadow-2xl overflow-hidden relative border border-white/10 m-4 ring-1 ring-white/5">
        
        {/* Sidebar */}
        <AnimatePresence>
          {isSidebarOpen && (
            <motion.div 
              initial={{ width: 0, opacity: 0 }} 
              animate={{ width: 260, opacity: 1 }} 
              exit={{ width: 0, opacity: 0 }} 
              className="h-full bg-[#0F0F13]/95 backdrop-blur-xl border-r border-white/5 flex flex-col shrink-0 z-20"
            >
              <div className="p-4 border-b border-white/5 flex items-center justify-between">
                <span className="text-[10px] font-bold text-gray-400 uppercase tracking-widest pl-2">Chat History</span>
                <button onClick={() => setIsSidebarOpen(false)} className="p-1.5 hover:bg-white/5 rounded-md transition-colors">
                   <ChevronRight className="w-4 h-4 rotate-180 text-gray-400" />
                </button>
              </div>
              
              <div className="p-3">
                 <button 
                  onClick={startNewSession} 
                  className="w-full p-3 rounded-xl bg-gradient-to-r from-purple-500/10 to-blue-500/10 hover:from-purple-500/20 hover:to-blue-500/20 border border-white/5 hover:border-white/10 text-white text-sm font-medium flex items-center justify-center gap-2 transition-all"
                >
                  <Plus className="w-4 h-4" /> New Chat
                </button>
              </div>

              <div className="flex-1 overflow-y-auto px-3 pb-3 space-y-1">
                {history.map(s => (
                  <button 
                    key={s.id} 
                    onClick={() => { setMessages(s.messages); setCurrentSessionId(s.id); }} 
                    className={`w-full p-3 text-left rounded-xl text-xs truncate transition-all border ${currentSessionId === s.id ? 'bg-white/10 text-white border-white/10' : 'text-gray-400 hover:bg-white/5 border-transparent'}`}
                  >
                    <div className="flex items-center gap-3">
                       <MessageSquare className="w-3 h-3 opacity-50 shrink-0" />
                       <span className="truncate">{s.title}</span>
                    </div>
                  </button>
                ))}
              </div>
              
              <div className="p-4 border-t border-white/5">
                <button onClick={() => { localStorage.clear(); setHistory([]); }} className="text-xs text-gray-500 hover:text-red-400 flex items-center gap-2 transition-colors w-full px-2">
                   <Trash2 className="w-3 h-3" /> Clear History
                </button>
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Main Content */}
        <div className="flex-1 flex flex-col bg-[#0F0F13]/80 relative">
          
          {/* Header */}
          <div className="h-14 flex items-center justify-between px-5 border-b border-white/5 backdrop-blur-md z-10 select-none" data-tauri-drag-region>
            <div className="flex items-center gap-4">
              {!isSidebarOpen && (
                <button onClick={() => setIsSidebarOpen(true)} className="p-2 hover:bg-white/5 rounded-lg text-gray-400 hover:text-white transition-colors">
                  <History className="w-5 h-5" />
                </button>
              )}
              <LuminaLogo className="w-6 h-6" withText={true} />
            </div>
            
            <div className="flex items-center gap-2">
               <button 
                 onClick={() => setIsWebSearchEnabled(!isWebSearchEnabled)} 
                 className={`h-8 px-3 rounded-lg text-[10px] font-bold border transition-all flex items-center gap-1.5 ${isWebSearchEnabled ? 'border-blue-500/30 bg-blue-500/10 text-blue-400' : 'border-white/5 bg-white/5 text-gray-500 hover:bg-white/10'}`}
               >
                <Globe className="w-3 h-3" /> WEB
               </button>
               
               <div className="h-4 w-[1px] bg-white/10 mx-1"></div>

               <select 
                 value={selectedModel} 
                 onChange={e => setSelectedModel(e.target.value)} 
                 className="h-8 bg-transparent text-gray-300 text-xs font-medium outline-none cursor-pointer hover:text-white transition-colors"
               >
                 {availableModels.map(m => <option key={m} value={m} className="bg-[#1A1A23] text-gray-300">{m}</option>)}
               </select>

               <button onClick={() => setShowSettings(true)} className="w-8 h-8 flex items-center justify-center hover:bg-white/5 rounded-lg text-gray-400 hover:text-white transition-colors ml-1">
                <Settings className="w-4 h-4" />
               </button>
            </div>
          </div>

          {/* Messages Area */}
          <div className="flex-1 overflow-y-auto p-6 space-y-6 scroll-smooth custom-scrollbar">
            {messages.length === 0 ? (
              <div className="h-full flex flex-col items-center justify-center space-y-8 animate-in fade-in duration-500">
                <div className="relative">
                  <div className="absolute inset-0 bg-gradient-to-tr from-purple-500/20 to-blue-500/20 blur-[80px] rounded-full"></div>
                  <LuminaLogo className="w-24 h-24" />
                </div>
                
                <div className="grid grid-cols-2 gap-3 max-w-lg w-full px-8">
                    <button onClick={handleGetProcesses} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/10 rounded-2xl text-left transition-all group">
                        <Cpu className="w-5 h-5 text-purple-400 mb-2 group-hover:scale-110 transition-transform" />
                        <div className="text-sm font-medium text-gray-200">System Check</div>
                        <div className="text-xs text-gray-500 mt-1">Analyze active processes</div>
                    </button>
                    <button onClick={() => setInput("Write a python script to parse JSON")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/10 rounded-2xl text-left transition-all group">
                        <Terminal className="w-5 h-5 text-green-400 mb-2 group-hover:scale-110 transition-transform" />
                        <div className="text-sm font-medium text-gray-200">Coding Help</div>
                        <div className="text-xs text-gray-500 mt-1">Generate scripts & debug</div>
                    </button>
                    <button onClick={() => setInput("Explain General Relativity")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/10 rounded-2xl text-left transition-all group">
                        <Sparkles className="w-5 h-5 text-blue-400 mb-2 group-hover:scale-110 transition-transform" />
                        <div className="text-sm font-medium text-gray-200">Learn</div>
                        <div className="text-xs text-gray-500 mt-1">Complex topics simplified</div>
                    </button>
                     <button onClick={() => setInput("Draft an email to my boss about a project")} className="p-4 bg-white/5 hover:bg-white/10 border border-white/5 hover:border-white/10 rounded-2xl text-left transition-all group">
                        <Command className="w-5 h-5 text-orange-400 mb-2 group-hover:scale-110 transition-transform" />
                        <div className="text-sm font-medium text-gray-200">Writing</div>
                        <div className="text-xs text-gray-500 mt-1">Emails, posts, and docs</div>
                    </button>
                </div>
              </div>
            ) : (
              messages.map((m, i) => (
                <motion.div 
                  initial={{ opacity: 0, y: 10 }}
                  animate={{ opacity: 1, y: 0 }}
                  transition={{ duration: 0.3 }}
                  key={i} 
                  className={`flex gap-4 ${m.role === 'user' ? 'flex-row-reverse' : ''}`}
                >
                  <div className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 shadow-lg ${m.role === 'user' ? 'bg-gradient-to-br from-purple-500 to-indigo-600' : 'bg-[#1A1A23] border border-white/10'}`}>
                    {m.role === 'user' ? <User className="w-4 h-4 text-white" /> : <Bot className="w-4 h-4 text-purple-300" />}
                  </div>
                  <div className={`max-w-[85%] p-4 rounded-2xl text-sm leading-relaxed shadow-sm ${m.role === 'user' ? 'bg-white/10 text-white backdrop-blur-sm' : 'bg-transparent text-gray-300'}`}>
                    <div className="prose prose-invert prose-sm max-w-none prose-p:leading-relaxed prose-pre:bg-[#0A0A0E] prose-pre:border prose-pre:border-white/10 prose-pre:rounded-xl">
                      <ReactMarkdown>{m.content}</ReactMarkdown>
                    </div>
                  </div>
                </motion.div>
              ))
            )}
            {isLoading && (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="flex gap-4">
                 <div className="w-8 h-8 rounded-lg bg-[#1A1A23] border border-white/10 flex items-center justify-center shrink-0">
                    <Bot className="w-4 h-4 text-purple-300" />
                 </div>
                 <div className="flex items-center gap-1 h-8">
                    <span className="w-1.5 h-1.5 bg-gray-600 rounded-full animate-bounce [animation-delay:-0.3s]"></span>
                    <span className="w-1.5 h-1.5 bg-gray-600 rounded-full animate-bounce [animation-delay:-0.15s]"></span>
                    <span className="w-1.5 h-1.5 bg-gray-600 rounded-full animate-bounce"></span>
                 </div>
              </motion.div>
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* Input Area */}
          <div className="p-5">
            <div className="max-w-3xl mx-auto relative group">
              <div className="absolute -inset-0.5 bg-gradient-to-r from-purple-500/50 to-blue-500/50 rounded-2xl opacity-0 group-focus-within:opacity-100 transition-opacity duration-500 blur-md"></div>
              <div className="relative flex items-center bg-[#15151A] rounded-xl border border-white/10 p-1.5 shadow-xl transition-colors group-focus-within:border-white/20">
                <input 
                  ref={inputRef}
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && !e.shiftKey && handleSendMessage()}
                  placeholder="Ask Lumina anything..."
                  className="flex-1 bg-transparent px-4 py-2.5 outline-none text-sm text-white placeholder-gray-500 font-medium"
                  autoFocus
                />
                <button 
                  onClick={handleSendMessage} 
                  disabled={isLoading || !input.trim()} 
                  className={`p-2.5 rounded-lg transition-all ${isLoading || !input.trim() ? 'bg-white/5 text-gray-600' : 'bg-white text-black hover:bg-gray-200 hover:scale-105 active:scale-95 shadow-lg shadow-white/10'}`}
                >
                  {isLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4 fill-current" />}
                </button>
              </div>
              <div className="text-center mt-2">
                  <p className="text-[10px] text-gray-600">Lumina runs locally. Generated content may be inaccurate.</p>
              </div>
            </div>
          </div>

          {/* Settings Overlay */}
          <AnimatePresence>
            {showSettings && (
              <motion.div 
                initial={{ opacity: 0 }} 
                animate={{ opacity: 1 }} 
                exit={{ opacity: 0 }} 
                className="absolute inset-0 z-50 bg-black/60 backdrop-blur-md flex items-center justify-center p-6"
                onClick={() => setShowSettings(false)}
              >
                <motion.div 
                  initial={{ scale: 0.95, y: 10 }} 
                  animate={{ scale: 1, y: 0 }} 
                  exit={{ scale: 0.95, y: 10 }} 
                  onClick={e => e.stopPropagation()}
                  className="bg-[#121216] border border-white/10 rounded-2xl w-full max-w-md p-6 shadow-2xl ring-1 ring-white/5"
                >
                  <div className="flex justify-between items-center mb-6">
                    <h2 className="text-lg font-bold text-white">Settings</h2>
                    <button onClick={() => setShowSettings(false)} className="w-8 h-8 flex items-center justify-center rounded-full hover:bg-white/10 transition-colors text-gray-400">✕</button>
                  </div>

                  <div className="space-y-6">
                    <div className="space-y-2">
                       <label className="text-xs font-bold text-gray-500 uppercase tracking-widest">Connection</label>
                       <div className="flex items-center justify-between p-3 bg-white/5 rounded-xl border border-white/5">
                          <div className="flex items-center gap-3">
                             <div className="w-8 h-8 rounded-lg bg-purple-500/20 flex items-center justify-center text-purple-400"><Terminal className="w-4 h-4"/></div>
                             <div>
                                <div className="text-sm font-medium text-white">Ollama Local</div>
                                <div className="text-[10px] text-gray-500">http://127.0.0.1:11434</div>
                             </div>
                          </div>
                          <div className="flex items-center gap-2 px-2 py-1 rounded-full bg-green-500/10 border border-green-500/20">
                             <div className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse"></div>
                             <span className="text-[10px] font-bold text-green-500">Active</span>
                          </div>
                       </div>
                    </div>
                  </div>

                  <div className="mt-8 flex justify-end">
                    <button 
                      onClick={() => setShowSettings(false)} 
                      className="px-6 py-2 bg-white text-black hover:bg-gray-200 rounded-lg text-xs font-bold transition-colors"
                    >
                      Done
                    </button>
                  </div>
                </motion.div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </div>
    </div>
  );
};

export default App;