import { AIProviderSelect } from './ai-provider-select'
import { useState, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Card } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import { Send, Loader2, Sparkles, CheckSquare, Square } from 'lucide-react'
import type { useChatHistory } from './use-chat-history'
import type { ExcalidrawWrapperRef, ElementSummary } from './wrapper'
import { MessageBubble } from './chat-message'
import { ChatPanelHeader } from './chat-panel-header'
import { useLocale } from '@/hooks/use-locale'
import type { useAiDraw } from './use-ai-draw'
import { useChatScroll } from './use-chat-scroll'
import { continuationPrompt } from '@/lib/agent/context'

type ChatHistoryApi = ReturnType<typeof useChatHistory>
type AiDrawApi = ReturnType<typeof useAiDraw>

interface ChatPanelProps {
  className?: string
  excalidrawRef?: React.RefObject<ExcalidrawWrapperRef | null>
  chatHistory: ChatHistoryApi
  aiDraw: AiDrawApi
  selectedElements: ElementSummary[]
  onClose?: () => void
}

export function ChatPanel({
  className,
  excalidrawRef,
  chatHistory,
  aiDraw,
  selectedElements,
  onClose,
}: ChatPanelProps) {
  const { t } = useLocale()
  const [input, setInput] = useState('')
  const [isComposing, setIsComposing] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const { currentSession, isLoaded, createSession } = chatHistory
  const { containerRef, onScroll, scrollToLatest, showLatest } = useChatScroll(currentSession)

  const { isLoading, send, abort, provider, setProvider } = aiDraw

  const handleSend = async () => {
    const value = input
    setInput('')
    const ok = await send(value)
    if (!ok) setInput(value)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey && !isComposing) {
      e.preventDefault()
      void handleSend()
    }
  }

  const handleNewChat = () => {
    const newSessionId = createSession()
    excalidrawRef?.current?.switchToSession(newSessionId, true)
  }

  if (!isLoaded) {
    return (
      <div className={cn('flex items-center justify-center', className)}>
        <Loader2 className="w-6 h-6 animate-spin text-primary" />
      </div>
    )
  }

  return (
    <div className={cn('flex h-full min-w-0 flex-col', className)}>
      <ChatPanelHeader
        chatHistory={chatHistory}
        excalidrawRef={excalidrawRef}
        onNewChat={handleNewChat}
        onClose={onClose}
      />
      <div className="flex-1 flex flex-col min-h-0 min-w-0">
        <div ref={containerRef} onScroll={onScroll} className="flex-1 min-h-0 overflow-y-auto p-4 space-y-4">
          {(!currentSession || currentSession.messages.length === 0) && (
            <div className="flex flex-col items-center justify-center h-full text-center text-foreground/50">
              <Sparkles className="w-12 h-12 mb-4 text-primary/30" />
              <p className="text-lg font-medium mb-2">{t.appTitle}</p>
              <p className="text-sm max-w-xs">{t.appSubtitle}</p>
              <div className="mt-6 space-y-2 text-xs text-foreground/40">
                <p>{t.tryThese}</p>
                <p>{t.example1}</p>
                <p>{t.example2}</p>
                <p>{t.example3}</p>
              </div>
            </div>
          )}

          {currentSession?.messages.map((message) => (
            <MessageBubble key={message.id} message={message} isStreaming={isLoading && message.id === currentSession.messages.at(-1)?.id}
              onContinue={run => { const prompt = continuationPrompt(run); void send(prompt, run.header.id) }} />
          ))}

          {isLoading && provider === 'api' && (
            <div className="flex items-center gap-2 text-foreground/50">
              <Loader2 className="w-4 h-4 animate-spin" />
              <span className="text-sm">{t.aiThinking}</span>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-xs text-foreground/50 hover:text-foreground"
                onClick={abort}
              >
                <Square className="w-3 h-3 fill-current" />
                {t.stop}
              </Button>
            </div>
          )}

        </div>
        {showLatest && <Button variant="ghost" size="sm" className="mx-auto my-1 shrink-0 text-xs" onClick={scrollToLatest}>{t.latestMessage}</Button>}

        {selectedElements.length > 0 && (
          <div className="px-3 py-2 bg-primary/10 border-b border-border">
            <div className="flex items-center gap-2 text-xs">
              <CheckSquare className="w-3.5 h-3.5 text-primary" />
              <span className="font-medium text-primary">
                {t.selectedCount(selectedElements.length)}
              </span>
            </div>
            <div className="mt-1.5 flex flex-wrap gap-1.5">
              {selectedElements.slice(0, 10).map((el) => (
                <span
                  key={el.id}
                  className="px-1.5 py-0.5 rounded bg-background border border-border text-[10px] font-mono text-foreground/70"
                >
                  {el.id.slice(0, 8)}
                </span>
              ))}
              {selectedElements.length > 10 && (
                <span className="px-1.5 py-0.5 text-[10px] text-foreground/50">
                  {t.andMore(selectedElements.length)}
                </span>
              )}
            </div>
          </div>
        )}

        <div className="p-3 border-t border-border bg-card">
          <div className="mb-2"><AIProviderSelect value={provider} onChange={setProvider} disabled={isLoading} compact /></div>
          <Card className="flex items-end gap-2 p-2 bg-secondary/5 border-border/50">
            <Textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={handleKeyDown}
              onCompositionStart={() => setIsComposing(true)}
              onCompositionEnd={() => setIsComposing(false)}
              placeholder={t.placeholder}
              className="min-h-[40px] max-h-[120px] resize-none border-0 bg-transparent focus-visible:ring-0 p-2"
              disabled={isLoading}
            />
            <Button
              size="icon"
              onClick={() => {
                if (isLoading) abort()
                else void handleSend()
              }}
              aria-label={isLoading ? t.stop : t.send}
              disabled={!isLoading && !input.trim()}
              className="shrink-0 w-9 h-9"
            >
              {isLoading ? (
                <Square className="w-4 h-4 fill-current" />
              ) : (
                <Send className="w-4 h-4" />
              )}
            </Button>
          </Card>
        </div>
      </div>
    </div>
  )
}
