import React, { useState, useEffect, useRef } from 'react';
import { 
    Send, Cpu, FileText, Terminal, Loader2, Bot, 
    Settings, Globe, History, Download, ChevronRight, MessageSquare, Plus, Trash2, User 
} from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import { motion, AnimatePresence } from 'framer-motion';
import { Message, ChatSession } from './types';
import { SYSTEM_PROMPT } from './src/constants';

// --- Mock Tauri Invoke for Browser Dev ---
const invoke = async <T,>(cmd: string, args?: any): Promise<T> => {
  if (window.__TAURI__) {
    return window.__TAURI__.invoke(cmd, args);
  }
  console.log(`[Mock Tauri] Invoking ${cmd}`, args);
  if (cmd === 'chat_with_ollama') return new Promise(resolve => setTimeout(() => resolve(`(Mock) I see you said: ${args.messages.at(-1).content}` as any), 1000));
  if (cmd === 'get_ollama_models') return ['llama3', 'mistral', 'gemma:2b', 'neutral'] as any;
  if (cmd === 'perform_web_search') return `Web Search Results: \n1. Information about ${args.query} found on the web.\n2. More details here.` as any;
  if (cmd === 'get_system_processes') return ['Chrome (500MB)', 'Code (200MB)'] as any;
  return null as any;
};

// --- Custom Logo Component ---
const LuminaLogo: React.FC<{ className?: string, withText?: boolean }> = ({ className = "w-8 h-8", withText = false }) => (
  <div className={`flex items-center gap-2 ${withText ? '' : 'justify-center'}`}>
    <svg className={className} viewBox="0 0 100 100" fill="none" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="lumina_grad" x1="0" y1="100" x2="100" y2="0" gradientUnits="userSpaceOnUse">
          <stop stopColor="#7C3AED" /> {/* Violet-600 */}
          <stop offset="1" stopColor="#3B82F6" /> {/* Blue-500 */}
        </linearGradient>
        <filter id="glow" x="-20" y="-20" width="140" height="140" filterUnits="userSpaceOnUse">
          <feGaussianBlur stdDeviation="5" result="blur" />
          <feComposite in="SourceGraphic" in2="blur" operator="over" />
        </filter>
      </defs>
      {/* Background shape (optional usage depending on context, keeping transparent for now or adding subtle backdrop) */}
      
      {/* The Spark/Star */}
      <path 
        d="M50 5 L58 35 L95 50 L58 65 L50 95 L42 65 L5 50 L42 35 Z" 
        fill="url(#lumina_grad)" 
        stroke="rgba(255,255,255,0.5)" 
        strokeWidth="2"
        filter="url(#glow)"
      />
      
      {/* Center Brightness */}
      <circle cx="50" cy="50" r="10" fill="white" fillOpacity="0.8" filter="url(#glow)" />
    </svg>
    {withText && (
      <span className="text-lg font-bold bg-clip-text text-transparent bg-gradient-to-r from-purple-200 to-blue-200 tracking-wide">
        Lumina
      </span>
    )}
  </div>
);

const App: React.FC = () => {
  // Chat State
  const [input, setInput] = useState('');
  const [messages, setMessages] = useState<Message[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [currentSessionId, setCurrentSessionId] = useState<string>(Date.now().toString());
  const [history, setHistory] = useState<ChatSession[]>([]);
  
  // Capability State
  const [isWebSearchEnabled, setIsWebSearchEnabled] = useState(false);
  const [selectedModel, setSelectedModel] = useState<string>('llama3');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  
  // UI State
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [newModelName, setNewModelName] = useState('');
  const [downloadStatus, setDownloadStatus] = useState('');

  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // --- Initialization & History ---

  useEffect(() => {
    // Load history
    const saved = localStorage.getItem('lumina_history');
    if (saved) {
      setHistory(JSON.parse(saved));
    }
    // Fetch models
    fetchModels();
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    // Auto-save history when messages change
    if (messages.length > 0) {
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
    }
    scrollToBottom();
  }, [messages, currentSessionId]);

  const fetchModels = async () => {
    try {
        const models = await invoke<string[]>('get_ollama_models');
        setAvailableModels(models);
        if (models.length > 0 && !models.includes(selectedModel)) {
            setSelectedModel(models[0]);
        }
    } catch (e) {
        console.error("Failed to fetch models", e);
    }
  };

  const scrollToBottom = () => messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });

  // --- Actions ---

  const handleSendMessage = async () => {
    if (!input.trim()) return;

    const userMsg: Message = { role: 'user', content: input };
    setMessages(prev => [...prev, userMsg]);
    setInput('');
    setIsLoading(true);

    try {
        let systemContext = SYSTEM_PROMPT;

        // 1. Web Search Capability
        if (isWebSearchEnabled) {
            try {
                const searchResults = await invoke<string>('perform_web_search', { query: userMsg.content });
                systemContext += `\n\n[Web Search Results Context]:\n${searchResults}\n\nUse this information to answer the user if relevant.`;
            } catch (err) {
                console.error("Search failed", err);
            }
        }

        // 2. Chat Request
        const conversationHistory = [
            { role: 'system', content: systemContext },
            ...messages,
            userMsg
        ];

        const response = await invoke<string>('chat_with_ollama', {
            model: selectedModel,
            messages: conversationHistory
        });

        const aiMsg: Message = { role: 'assistant', content: response };
        setMessages(prev => [...prev, aiMsg]);

    } catch (error) {
        setMessages(prev => [...prev, { role: 'assistant', content: `**Error:** ${error}` }]);
    } finally {
        setIsLoading(false);
    }
  };

  const startNewChat = () => {
      setMessages([]);
      setCurrentSessionId(Date.now().toString());
      setIsSidebarOpen(false);
      inputRef.current?.focus();
  };

  const loadSession = (session: ChatSession) => {
      setMessages(session.messages);
      setCurrentSessionId(session.id);
      setIsSidebarOpen(false);
  };

  const handleDownloadModel = async () => {
      if(!newModelName) return;
      setDownloadStatus('Pulling model (this may take a while)...');
      try {
          await invoke('pull_model', { name: newModelName });
          setDownloadStatus('Success!');
          setNewModelName('');
          fetchModels();
          setTimeout(() => setDownloadStatus(''), 3000);
      } catch (e) {
          setDownloadStatus(`Error: ${e}`);
      }
  };

  const handleGetProcesses = async () => {
    setIsLoading(true);
    setMessages(prev => [...prev, { role: 'user', content: "Analyze current system processes." }]);
    try {
      const procs = await invoke<string[]>('get_system_processes');
      const processList = procs.join('\n- ');
      
      const contextMsg = `Here is the current process list:\n- ${processList}\n\nAnalyze this briefly.`;
      
      const response = await invoke<string>('chat_with_ollama', {
        model: selectedModel,
        messages: [{ role: 'system', content: SYSTEM_PROMPT }, { role: 'user', content: contextMsg }]
      });

      setMessages(prev => [...prev, { role: 'assistant', content: response }]);
    } catch (e) {
      setMessages(prev => [...prev, { role: 'assistant', content: "Failed to fetch processes." }]);
    } finally {
      setIsLoading(false);
    }
  };

  // --- Render ---

  return (
    <div className="h-screen w-screen flex items-center justify-center bg-transparent font-sans">
      <div className="w-full h-full md:w-[850px] md:h-[650px] glass-panel rounded-2xl flex shadow-2xl overflow-hidden text-white relative">
        
        {/* Sidebar (History) */}
        <AnimatePresence>
            {isSidebarOpen && (
                <motion.div 
                    initial={{ width: 0, opacity: 0 }} 
                    animate={{ width: 250, opacity: 1 }} 
                    exit={{ width: 0, opacity: 0 }}
                    className="h-full bg-black/40 backdrop-blur-md border-r border-white/10 flex flex-col z-20"
                >
                    <div className="p-4 border-b border-white/10 flex justify-between items-center">
                        <span className="text-sm font-semibold text-purple-200 flex items-center gap-2">
                            <History className="w-4 h-4" /> History
                        </span>
                        <button onClick={() => setIsSidebarOpen(false)} className="hover:bg-white/10 p-1 rounded">
                            <ChevronRight className="w-4 h-4 rotate-180" />
                        </button>
                    </div>
                    <div className="flex-1 overflow-y-auto p-2 space-y-1">
                        <button onClick={startNewChat} className="w-full flex items-center gap-2 p-3 rounded-lg bg-purple-600/20 hover:bg-purple-600/40 text-sm text-purple-200 transition-colors mb-4 border border-purple-500/30">
                            <Plus className="w-4 h-4" /> New Chat
                        </button>
                        {history.map(session => (
                            <button 
                                key={session.id} 
                                onClick={() => loadSession(session)}
                                className={`w-full text-left p-3 rounded-lg text-xs truncate transition-colors ${currentSessionId === session.id ? 'bg-white/10 text-white' : 'text-gray-400 hover:bg-white/5 hover:text-gray-200'}`}
                            >
                                {session.title || 'Untitled Chat'}
                            </button>
                        ))}
                    </div>
                    <div className="p-4 border-t border-white/10 text-xs text-center text-gray-500">
                        Lumina v0.1.0
                    </div>
                </motion.div>
            )}
        </AnimatePresence>

        {/* Main Content */}
        <div className="flex-1 flex flex-col relative bg-gradient-to-br from-[#0F0F13] to-[#14141c]">
            
            {/* Header */}
            <div className="h-14 border-b border-white/10 flex items-center justify-between px-4 bg-white/5 select-none drag-region">
                <div className="flex items-center gap-3">
                    <button onClick={() => setIsSidebarOpen(!isSidebarOpen)} className={`p-2 rounded-lg hover:bg-white/10 transition-colors ${isSidebarOpen ? 'text-purple-400' : 'text-gray-400'}`}>
                        {isSidebarOpen ? <ChevronRight className="w-5 h-5" /> : <History className="w-5 h-5" />}
                    </button>
                    
                    {/* Replaced Icon/Text with Logo Component */}
                    <LuminaLogo className="w-8 h-8" withText={true} />
                </div>

                <div className="flex items-center gap-3">
                    {/* Web Search Toggle */}
                    <button 
                        onClick={() => setIsWebSearchEnabled(!isWebSearchEnabled)}
                        className={`flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium border transition-all ${isWebSearchEnabled ? 'bg-blue-500/20 border-blue-500/50 text-blue-300 shadow-[0_0_10px_rgba(59,130,246,0.3)]' : 'bg-white/5 border-white/10 text-gray-400 hover:bg-white/10'}`}
                    >
                        <Globe className="w-3 h-3" />
                        {isWebSearchEnabled ? 'Web On' : 'Web Off'}
                    </button>

                    {/* Model Selector */}
                    <div className="relative group">
                         <select 
                            value={selectedModel}
                            onChange={(e) => setSelectedModel(e.target.value)}
                            className="appearance-none bg-black/30 border border-white/10 text-xs text-gray-300 rounded-lg pl-3 pr-8 py-1.5 focus:outline-none focus:border-purple-500 cursor-pointer hover:bg-black/50 transition-colors"
                         >
                            {availableModels.length > 0 ? (
                                availableModels.map(m => <option key={m} value={m}>{m}</option>)
                            ) : (
                                <option value="loading">Loading models...</option>
                            )}
                         </select>
                         <div className="absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none text-gray-500">
                            <ChevronRight className="w-3 h-3 rotate-90" />
                         </div>
                    </div>

                    <button onClick={() => setShowSettings(true)} className="p-2 text-gray-400 hover:text-white hover:bg-white/10 rounded-lg transition-colors">
                        <Settings className="w-4 h-4" />
                    </button>
                </div>
            </div>

            {/* Chat Area */}
            <div className="flex-1 overflow-y-auto p-4 space-y-4 scroll-smooth">
                {messages.length === 0 ? (
                    <div className="h-full flex flex-col items-center justify-center opacity-60 space-y-6">
                        <div className="relative">
                            <div className="w-32 h-32 rounded-full bg-purple-500/20 blur-3xl absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 animate-pulse-slow"></div>
                            
                            {/* Replaced Icon with Logo Component (Larger) */}
                            <LuminaLogo className="w-20 h-20 relative z-10" />
                        </div>
                        <div className="text-center space-y-2">
                            <h2 className="text-2xl font-light text-white/90">Lumina AI</h2>
                            <p className="text-sm text-gray-500">Local. Private. Powerful.</p>
                        </div>
                        <div className="flex gap-3">
                            <button onClick={handleGetProcesses} className="capability-btn"><Cpu className="w-4 h-4" /> Check Processes</button>
                            <button onClick={() => setIsWebSearchEnabled(true)} className="capability-btn"><Globe className="w-4 h-4" /> Search Web</button>
                        </div>
                    </div>
                ) : (
                    <div className="space-y-6">
                        {messages.map((msg, idx) => (
                            <motion.div
                                key={idx}
                                initial={{ opacity: 0, y: 10 }}
                                animate={{ opacity: 1, y: 0 }}
                                className={`flex gap-4 ${msg.role === 'user' ? 'flex-row-reverse' : ''}`}
                            >
                                <div className={`w-8 h-8 rounded-full flex items-center justify-center flex-shrink-0 ${msg.role === 'user' ? 'bg-purple-500/20' : 'bg-blue-500/20'}`}>
                                    {msg.role === 'user' ? <User className="w-4 h-4 text-purple-400" /> : <LuminaLogo className="w-5 h-5" />}
                                </div>
                                <div className={`max-w-[80%] rounded-2xl px-5 py-3.5 text-sm leading-relaxed ${msg.role === 'user' ? 'bg-purple-600 text-white' : 'bg-white/5 border border-white/5 text-gray-200'}`}>
                                    {msg.role === 'assistant' ? (
                                        <div className="prose prose-invert prose-sm max-w-none prose-p:my-1 prose-pre:bg-black/50 prose-pre:p-3 prose-pre:rounded-lg">
                                            <ReactMarkdown>
                                                {msg.content}
                                            </ReactMarkdown>
                                        </div>
                                    ) : (
                                        <span>{msg.content}</span>
                                    )}
                                </div>
                            </motion.div>
                        ))}
                        {isLoading && (
                            <div className="flex gap-4">
                                <div className="w-8 h-8 rounded-full bg-blue-500/20 flex items-center justify-center"><LuminaLogo className="w-4 h-4" /></div>
                                <div className="flex items-center space-x-2 text-gray-500 text-xs h-8">
                                    <Loader2 className="w-3 h-3 animate-spin" />
                                    <span>Thinking...</span>
                                </div>
                            </div>
                        )}
                        <div ref={messagesEndRef} />
                    </div>
                )}
            </div>

            {/* Input Area */}
            <div className="p-4 bg-gradient-to-t from-black/80 via-black/40 to-transparent backdrop-blur-sm">
                <div className="relative group">
                    <div className="absolute -inset-0.5 bg-gradient-to-r from-purple-500 to-blue-500 rounded-xl opacity-20 group-hover:opacity-40 transition duration-500 blur"></div>
                    <div className="relative flex items-center bg-[#1A1A23] rounded-xl border border-white/10 focus-within:border-purple-500/50 transition-colors shadow-lg">
                        <input
                            ref={inputRef}
                            type="text"
                            value={input}
                            onChange={(e) => setInput(e.target.value)}
                            onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSendMessage()}
                            placeholder={isWebSearchEnabled ? "Ask Lumina to search the web..." : "Ask Lumina anything..."}
                            className="flex-1 bg-transparent text-white px-4 py-3.5 focus:outline-none placeholder-gray-500 text-sm"
                            autoFocus
                        />
                        <button 
                            onClick={handleSendMessage}
                            disabled={!input.trim() || isLoading}
                            className="mr-2 p-2 rounded-lg text-gray-400 hover:text-white hover:bg-white/10 transition-all disabled:opacity-50"
                        >
                            <Send className="w-4 h-4" />
                        </button>
                    </div>
                </div>
                <div className="flex justify-between mt-2 px-1 text-[10px] text-gray-600 uppercase tracking-wider font-semibold">
                    <span className="flex items-center gap-1">{isWebSearchEnabled && <Globe className="w-3 h-3 text-blue-500" />} {selectedModel}</span>
                    <span className="flex items-center gap-1"><Terminal className="w-3 h-3"/> Secure</span>
                </div>
            </div>

            {/* Settings Modal */}
            <AnimatePresence>
                {showSettings && (
                    <motion.div 
                        initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}
                        className="absolute inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-8"
                    >
                        <motion.div 
                            initial={{ scale: 0.95 }} animate={{ scale: 1 }} exit={{ scale: 0.95 }}
                            className="bg-[#1A1A23] border border-white/10 rounded-2xl w-full max-w-md p-6 shadow-2xl"
                        >
                            <div className="flex justify-between items-center mb-6">
                                <h3 className="text-lg font-medium text-white">Settings</h3>
                                <button onClick={() => setShowSettings(false)} className="text-gray-400 hover:text-white">✕</button>
                            </div>
                            
                            <div className="space-y-4">
                                <div>
                                    <label className="block text-xs font-medium text-gray-400 mb-2">Available Models</label>
                                    <div className="bg-black/30 rounded-lg p-3 max-h-32 overflow-y-auto border border-white/5 space-y-1">
                                        {availableModels.map(m => (
                                            <div key={m} className="text-sm text-gray-300 flex items-center gap-2">
                                                <div className="w-1.5 h-1.5 rounded-full bg-green-500"></div> {m}
                                            </div>
                                        ))}
                                    </div>
                                </div>

                                <div>
                                    <label className="block text-xs font-medium text-gray-400 mb-2">Download New Model</label>
                                    <div className="flex gap-2">
                                        <input 
                                            type="text" 
                                            value={newModelName}
                                            onChange={(e) => setNewModelName(e.target.value)}
                                            placeholder="e.g. llama3, mistral"
                                            className="flex-1 bg-black/30 border border-white/10 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-purple-500"
                                        />
                                        <button 
                                            onClick={handleDownloadModel}
                                            disabled={!newModelName}
                                            className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white rounded-lg text-sm font-medium transition-colors disabled:opacity-50 flex items-center gap-2"
                                        >
                                            <Download className="w-4 h-4" /> Pull
                                        </button>
                                    </div>
                                    {downloadStatus && (
                                        <p className={`text-xs mt-2 ${downloadStatus.includes('Error') ? 'text-red-400' : 'text-green-400'}`}>
                                            {downloadStatus}
                                        </p>
                                    )}
                                </div>
                            </div>
                            <div className="mt-6 pt-4 border-t border-white/10 flex justify-end">
                                <button onClick={() => setShowSettings(false)} className="px-4 py-2 bg-white/5 hover:bg-white/10 text-white rounded-lg text-sm transition-colors">
                                    Done
                                </button>
                            </div>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>

        </div>
      </div>
      
      <style>{`
        .capability-btn {
            @apply flex items-center gap-2 px-4 py-2 bg-white/5 hover:bg-white/10 rounded-xl text-sm transition-colors border border-white/5 text-gray-300 hover:text-white;
        }
      `}</style>
    </div>
  );
};

export default App;