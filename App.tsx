import React, { useState, useEffect, useRef } from 'react';
import { 
    Send, Cpu, FileText, Terminal, Loader2, Bot, Sparkles,
    Settings, Globe, History, Download, ChevronRight, MessageSquare, Plus, Trash2, User
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { motion, AnimatePresence } from 'framer-motion';
import { Message, ChatSession } from './types';
import { SYSTEM_PROMPT } from './src/constants';

// --- Mock Tauri Invoke ---
const invoke = async <T,>(cmd: string, args?: any): Promise<T> => {
  if (window.__TAURI__) return window.__TAURI__.invoke(cmd, args);
  if (cmd === 'chat_with_ollama') return new Promise(r => setTimeout(() => r(`(Mock) Response to: ${args.messages.at(-1).content}` as any), 800));
  if (cmd === 'get_ollama_models') return ['llama3', 'mistral', 'phi3'] as any;
  if (cmd === 'get_system_processes') return ['Chrome (500MB)', 'Code (200MB)'] as any;
  return null as any;
};

// --- Enhanced Vector Logo (SVG) ---
// Kept the new design as requested
const LuminaLogo: React.FC<{ className?: string, withText?: boolean }> = ({ className = "w-8 h-8", withText = false }) => (
  <div className={`flex items-center gap-2 ${withText ? '' : 'justify-center'}`}>
    <svg className={className} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="logo_grad_main" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" stopColor="#A78BFA" />
          <stop offset="100%" stopColor="#3B82F6" />
        </linearGradient>
        <filter id="neon_glow" x="-50%" y="-50%" width="200%" height="200%">
          <feGaussianBlur stdDeviation="4" result="blur" />
          <feComposite in="SourceGraphic" in2="blur" operator="over" />
        </filter>
      </defs>
      <path 
        d="M50 10 L56 44 L90 50 L56 56 L50 90 L44 56 L10 50 L44 44 Z" 
        fill="url(#logo_grad_main)" 
        filter="url(#neon_glow)"
      />
      <circle cx="50" cy="50" r="6" fill="white" className="animate-pulse" />
    </svg>
    {withText && (
      <span className="text-xl font-bold tracking-tight bg-clip-text text-transparent bg-gradient-to-r from-white via-purple-200 to-blue-200">
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
  
  // Feature toggle state (kept local only)
  const [isWebSearchEnabled, setIsWebSearchEnabled] = useState(false); 

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const saved = localStorage.getItem('lumina_history');
    if (saved) setHistory(JSON.parse(saved));
    fetchModels();
  }, []);

  useEffect(() => {
    if (messages.length > 0) {
      scrollToBottom();
      saveHistory();
    }
  }, [messages]);

  const saveHistory = () => {
    setHistory(prev => {
        const existingIndex = prev.findIndex(s => s.id === currentSessionId);
        const title = messages[0].content.slice(0, 30) + (messages[0].content.length > 30 ? '...' : '');
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

  const scrollToBottom = () => messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });

  const fetchModels = async () => {
    const models = await invoke<string[]>('get_ollama_models');
    if (models) setAvailableModels(models);
  };

  const handleSendMessage = async () => {
    if (!input.trim() || isLoading) return;
    const userMsg: Message = { role: 'user', content: input };
    setMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);

    try {
        let context = SYSTEM_PROMPT;
        
        // Note: Real web search would require a separate local tool or API, 
        // for now we stick to pure LLM generation to keep it local/free as requested.
        
        const response = await invoke<string>('chat_with_ollama', {
          model: selectedModel,
          messages: [{role: 'system', content: context}, ...messages, userMsg]
        });
        
        setMessages(prev => [...prev, { role: 'assistant', content: response }]);
    } catch (e: any) {
      console.error("Chat failed", e);
      setMessages(prev => [...prev, { role: 'assistant', content: "Error communicating with Ollama. Make sure it is running." }]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleGetProcesses = async () => {
    setIsLoading(true);
    // Add a ghost message to UI
    const sysMsg: Message = { role: 'user', content: "Check system processes" };
    setMessages(prev => [...prev, sysMsg]);
    
    try {
        const procs = await invoke<string[]>('get_system_processes');
        const context = `System Processes:\n${procs.join('\n')}\n\nAnalyze these briefly.`;
        const response = await invoke<string>('chat_with_ollama', {
            model: selectedModel,
            messages: [{role: 'system', content: SYSTEM_PROMPT}, { role: 'user', content: context }]
        });
        setMessages(prev => [...prev, { role: 'assistant', content: response }]);
    } catch (e) {
        setMessages(prev => [...prev, { role: 'assistant', content: "Failed to retrieve system processes." }]);
    } finally {
        setIsLoading(false);
    }
  };

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent font-sans">
      <div className="w-full h-full md:w-[900px] md:h-[680px] glass-panel rounded-3xl flex shadow-2xl overflow-hidden text-white relative border border-white/10">
        
        {/* Sidebar */}
        <AnimatePresence>
          {isSidebarOpen && (
            <motion.div initial={{ width: 0 }} animate={{ width: 280 }} exit={{ width: 0 }} className="h-full bg-black/40 backdrop-blur-xl border-r border-white/5 flex flex-col overflow-hidden">
              <div className="p-4 border-b border-white/5 flex items-center justify-between">
                <span className="text-xs font-bold text-gray-500 uppercase tracking-widest">Chat History</span>
                <button onClick={() => setIsSidebarOpen(false)}><ChevronRight className="w-4 h-4 rotate-180 text-gray-400" /></button>
              </div>
              <div className="flex-1 p-2 overflow-y-auto space-y-1">
                <button onClick={() => { setMessages([]); setCurrentSessionId(Date.now().toString()); }} className="w-full p-3 rounded-xl bg-purple-500/10 hover:bg-purple-500/20 text-purple-300 text-sm flex items-center gap-2 transition-all mb-4">
                  <Plus className="w-4 h-4" /> New Session
                </button>
                {history.map(s => (
                  <button key={s.id} onClick={() => { setMessages(s.messages); setCurrentSessionId(s.id); }} className="w-full p-3 text-left rounded-lg text-xs text-gray-400 hover:bg-white/5 truncate">{s.title}</button>
                ))}
              </div>
            </motion.div>
          )}
        </AnimatePresence>

        {/* Main Content */}
        <div className="flex-1 flex flex-col bg-[#0F0F13]/90 relative">
          
          {/* Header */}
          <div className="h-16 flex items-center justify-between px-6 border-b border-white/5 backdrop-blur-md">
            <div className="flex items-center gap-4">
              <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className="p-2 hover:bg-white/5 rounded-lg transition-colors">
                <History className="w-5 h-5 text-gray-400" />
              </button>
              <LuminaLogo withText={true} />
            </div>
            <div className="flex items-center gap-3">
               <button onClick={() => setIsWebSearchEnabled(!isWebSearchEnabled)} className={`px-3 py-1.5 rounded-full text-[10px] font-bold border transition-all ${isWebSearchEnabled ? 'border-blue-500/50 bg-blue-500/10 text-blue-400 shadow-[0_0_15px_rgba(59,130,246,0.2)]' : 'border-white/10 text-gray-500'}`}>
                <Globe className="w-3 h-3 inline mr-1" /> WEB
               </button>
               <select value={selectedModel} onChange={e => setSelectedModel(e.target.value)} className="bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-xs focus:outline-none">
                 {availableModels.map(m => <option key={m} value={m}>{m}</option>)}
               </select>
               <button onClick={() => setShowSettings(true)} className="p-2 hover:bg-white/5 rounded-lg text-gray-400 transition-colors">
                <Settings className="w-4 h-4" />
               </button>
            </div>
          </div>

          {/* Chat Container */}
          <div className="flex-1 overflow-y-auto p-6 space-y-6">
            {messages.length === 0 ? (
              <div className="h-full flex flex-col items-center justify-center space-y-8 animate-in fade-in duration-700">
                <div className="relative group">
                  <div className="absolute inset-0 bg-purple-500/20 blur-[100px] rounded-full group-hover:bg-purple-500/40 transition-all duration-1000"></div>
                  <LuminaLogo className="w-32 h-32 relative drop-shadow-[0_0_30px_rgba(167,139,250,0.5)]" />
                </div>
                <div className="text-center space-y-2">
                  <h1 className="text-3xl font-light tracking-tight text-white/90">How can I help you, Master?</h1>
                  <p className="text-gray-500 text-sm">Powered by Local LLMs</p>
                </div>
                 <div className="flex gap-3">
                    <button onClick={handleGetProcesses} className="flex items-center gap-2 px-4 py-2 bg-white/5 hover:bg-white/10 rounded-xl text-sm transition-colors border border-white/5 text-gray-300 hover:text-white">
                        <Cpu className="w-4 h-4" /> Check Processes
                    </button>
                 </div>
              </div>
            ) : (
              messages.map((m, i) => (
                <div key={i} className={`flex gap-4 ${m.role === 'user' ? 'flex-row-reverse' : ''}`}>
                  <div className={`w-10 h-10 rounded-2xl flex items-center justify-center shrink-0 border ${m.role === 'user' ? 'bg-purple-500/10 border-purple-500/20' : 'bg-blue-500/10 border-blue-500/20'}`}>
                    {m.role === 'user' ? <User className="w-5 h-5 text-purple-400" /> : <LuminaLogo className="w-6 h-6" />}
                  </div>
                  <div className={`max-w-[75%] p-4 rounded-2xl text-sm leading-relaxed ${m.role === 'user' ? 'bg-purple-600/90 text-white shadow-lg' : 'bg-white/5 border border-white/10 text-gray-200'}`}>
                    <div className="prose prose-invert prose-sm max-w-none">
                      <ReactMarkdown>{m.content}</ReactMarkdown>
                    </div>
                  </div>
                </div>
              ))
            )}
            <div ref={messagesEndRef} />
          </div>

          {/* Input */}
          <div className="p-6 bg-gradient-to-t from-[#0F0F13] to-transparent">
            <div className="max-w-3xl mx-auto relative group">
              <div className="absolute -inset-0.5 bg-gradient-to-r from-purple-500 to-blue-500 rounded-2xl opacity-20 blur group-focus-within:opacity-50 transition-all duration-500"></div>
              <div className="relative flex items-center bg-[#1A1A23] rounded-2xl border border-white/10 p-2 shadow-2xl">
                <input 
                  ref={inputRef}
                  value={input}
                  onChange={e => setInput(e.target.value)}
                  onKeyDown={e => e.key === 'Enter' && handleSendMessage()}
                  placeholder="Tell me what's on your mind..."
                  className="flex-1 bg-transparent px-4 py-3 outline-none text-sm placeholder-gray-600"
                  autoFocus
                />
                <button onClick={handleSendMessage} disabled={isLoading || !input.trim()} className="p-3 bg-purple-600 hover:bg-purple-500 rounded-xl transition-all disabled:opacity-50">
                  {isLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : <Send className="w-4 h-4" />}
                </button>
              </div>
            </div>
          </div>

          {/* Settings Modal - Cleaned up */}
          <AnimatePresence>
            {showSettings && (
              <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="absolute inset-0 z-50 bg-black/80 backdrop-blur-md flex items-center justify-center p-6">
                <motion.div initial={{ scale: 0.9, y: 20 }} animate={{ scale: 1, y: 0 }} exit={{ scale: 0.9, y: 20 }} className="bg-[#1A1A23] border border-white/10 rounded-3xl w-full max-w-xl p-8 shadow-2xl">
                  <div className="flex justify-between items-center mb-8">
                    <h2 className="text-xl font-bold flex items-center gap-2"><Settings className="w-5 h-5 text-purple-400" /> System Settings</h2>
                    <button onClick={() => setShowSettings(false)} className="text-gray-500 hover:text-white transition-colors">✕</button>
                  </div>

                  <div className="space-y-6">
                    <section className="space-y-3">
                       <h3 className="text-xs font-bold text-gray-500 uppercase tracking-widest">Model Info</h3>
                       <div className="grid grid-cols-2 gap-4">
                         <div className="p-4 bg-white/5 rounded-xl border border-white/5">
                            <p className="text-[10px] text-gray-500 mb-1">Current Engine</p>
                            <p className="text-sm font-bold text-purple-300">Ollama Local</p>
                         </div>
                         <div className="p-4 bg-white/5 rounded-xl border border-white/5">
                            <p className="text-[10px] text-gray-500 mb-1">Status</p>
                            <p className="text-sm font-bold text-green-400 flex items-center gap-2">
                              <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse" /> Active
                            </p>
                         </div>
                       </div>
                    </section>
                  </div>

                  <div className="mt-8 flex justify-end">
                    <button onClick={() => setShowSettings(false)} className="px-6 py-2 bg-white/5 hover:bg-white/10 rounded-xl text-sm transition-all border border-white/10">Done</button>
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