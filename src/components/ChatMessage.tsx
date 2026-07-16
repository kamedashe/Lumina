import React from 'react';
import ReactMarkdown from 'react-markdown';
import { User, Wrench, Check, AlertTriangle, ChevronDown } from 'lucide-react';
import { Message, ToolInvocation } from '../types';
import { LuminaLogo } from './UI';

const ToolBadge: React.FC<{ tool: ToolInvocation }> = ({ tool }) => {
    const [open, setOpen] = React.useState(false);
    return (
        <div className="rounded-lg border border-border bg-bg/40 overflow-hidden">
            <button
                onClick={() => setOpen((o) => !o)}
                className="w-full flex items-center gap-2 px-3 py-2 text-xs text-muted hover:text-fg transition-colors"
            >
                {tool.isError ? (
                    <AlertTriangle size={13} className="text-red-400 shrink-0" />
                ) : (
                    <Check size={13} className="text-accent shrink-0" />
                )}
                <Wrench size={12} className="shrink-0 opacity-60" />
                <span className="font-mono font-medium">{tool.name}</span>
                <ChevronDown
                    size={13}
                    className={`ml-auto shrink-0 transition-transform ${open ? 'rotate-180' : ''}`}
                />
            </button>
            {open && (
                <pre className="px-3 pb-3 text-[11px] leading-relaxed font-mono text-subtle whitespace-pre-wrap max-h-56 overflow-auto custom-scrollbar">
                    {tool.content || '(пусто)'}
                </pre>
            )}
        </div>
    );
};

type Props = {
    message: Message;
    /** true, если этот ответ ассистента прямо сейчас стримится. */
    streaming?: boolean;
};

export const ChatMessage: React.FC<Props> = ({ message, streaming }) => {
    const isUser = message.role === 'user';

    return (
        <div className="flex gap-3.5">
            <div
                className={`w-7 h-7 rounded-lg flex items-center justify-center shrink-0 mt-0.5 ${
                    isUser ? 'bg-elevated border border-border' : ''
                }`}
            >
                {isUser ? <User size={15} className="text-muted" /> : <LuminaLogo className="w-7 h-7" />}
            </div>

            <div className="flex-1 min-w-0 space-y-2.5">
                <div className="text-xs font-semibold text-subtle">{isUser ? 'Вы' : 'Lumina'}</div>

                {message.tools && message.tools.length > 0 && (
                    <div className="space-y-1.5">
                        {message.tools.map((t) => (
                            <ToolBadge key={t.id} tool={t} />
                        ))}
                    </div>
                )}

                {(message.content || !isUser) && (
                    <div
                        className={`prose-chat text-fg ${
                            streaming && !message.content ? '' : ''
                        } ${streaming ? 'stream-caret' : ''}`}
                    >
                        <ReactMarkdown>{message.content}</ReactMarkdown>
                    </div>
                )}
            </div>
        </div>
    );
};
