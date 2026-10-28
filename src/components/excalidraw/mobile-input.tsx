import { AIProviderSelect } from './ai-provider-select'
import { useState, useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { Card } from '@/components/ui/card'
import {
  Send,
  Loader2,
  Trash2,
  Square,
  Plus,
  MessageSquare,
  ChevronUp,
  ChevronDown,
} from 'lucide-react'
import type { useChatHistory } from './use-chat-history'
import type { ExcalidrawWrapperRef, ElementSummary } from './wrapper'
import type { useAiDraw } from './use-ai-draw'
import { MessageBubble } from './chat-message'
import { useLocale } from '@/hooks/use-locale'
import { cn } from '@/lib/utils'
import { useChatScroll } from './use-chat-scroll'
import { continuationPrompt } from '@/lib/agent/context'

type ChatHistoryApi = ReturnType<typeof useChatHistory>
type AiDrawApi = ReturnType<typeof useAiDraw>

interface MobileInputProps {
  excalidrawRef: React.RefObject<ExcalidrawWrapperRef | null>
  chatHistory: ChatHistoryApi
  aiDraw: AiDrawApi
  selectedElements: ElementSummary[]
  onClearCanvas?: () => void
}

export function MobileInput({
  excalidrawRef,
  chatHistory,
  aiDraw,
  selectedElements,
  onClearCanvas,
}: MobileInputProps) {
  const { t } = useLocale()
  const [input, setInput] = useState('')
  const [isComposing, setIsComposing] = useState(false)
  const [showHistory, setShowHistory] = useState(false)
  const [showSessions, setShowSessions] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  const {
    sessions,
    currentSession,
    currentSessionId,
    createSession,
    switchSession,
  } = chatHistory
  const { isLoading, send, abort, provider, setProvider } = aiDraw
  const { containerRef, onScroll, scrollToLatest, showLatest } = useChatScroll(currentSession, showHistory)

  const handleSend = async () => {
    const value = input
    if (!value.trim() || isLoading) return
    setInput('')
    setShowHistory(true)
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
    excalidrawRef.current?.switchToSession(newSessionId, true)
    setShowSessions(false)
  }

  return (
    <div className="border-t border-border bg-card/95 backdrop-blur supports-[backdrop-filter]:bg-card/80 safe-area-inset-bottom">
      {showHistory && (
        <div ref={containerRef} onScroll={onScroll} className="max-h-48 overflow-y-auto border-b border-border px-3 py-2 space-y-2">
          {currentSession?.messages.slice(-8).map((message) => (
            <MessageBubble key={message.id} message={message} isStreaming={isLoading && message.id === currentSession.messages.at(-1)?.id} onContinue={run=>{void send(continuationPrompt(run),run.header.id)}} />
          ))}
        </div>
      )}
      {showHistory && showLatest && <Button variant="ghost" size="sm" className="mx-auto flex text-xs" onClick={scrollToLatest}>{t.latestMessage}</Button>}

      {showSessions && (
        <div className="max-h-40 overflow-y-auto border-b border-border px-3 py-2 space-y-1">
          <Button
            variant="outline"
            size="sm"
            onClick={handleNewChat}
            className="w-full mb-1 gap-2"
          >
            <Plus className="w-3.5 h-3.5" />
            {t.newChat}
          </Button>
          {sessions.map((session) => (
            <button
              key={session.id}
              type="button"
              className={cn(
                'w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-left text-xs',
                session.id === currentSessionId
                  ? 'bg-primary/10 text-primary'
                  : 'hover:bg-secondary/50'
              )}
              onClick={() => {
                switchSession(session.id)
                excalidrawRef.current?.switchToSession(
                  session.id,
                  session.useIndependentCanvas ?? false
                )
                setShowSessions(false)
              }}
            >
              <MessageSquare className="w-3.5 h-3.5 shrink-0" />
              <span className="truncate">{session.title}</span>
            </button>
          ))}
        </div>
      )}

      {selectedElements.length > 0 && (
        <div className="px-3 py-1.5 text-xs text-primary bg-primary/10 border-b border-border">
          {t.selectedCount(selectedElements.length)}
        </div>
      )}

      <div className="flex items-center gap-1 px-2 pt-2">
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          onClick={() => {
            setShowHistory((v) => !v)
            setShowSessions(false)
          }}
        >
          {showHistory ? (
            <ChevronDown className="w-3.5 h-3.5 mr-1" />
          ) : (
            <ChevronUp className="w-3.5 h-3.5 mr-1" />
          )}
          {t.appTitle}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 px-2 text-xs"
          onClick={() => {
            setShowSessions((v) => !v)
            setShowHistory(false)
          }}
        >
          <MessageSquare className="w-3.5 h-3.5 mr-1" />
          {t.newChat}
        </Button>
        {isLoading && provider === 'api' && (
          <div className="ml-auto flex items-center gap-1 text-xs text-foreground/50">
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
            {t.aiDrawing}
          </div>
        )}
      </div>

      <div className="p-3 pt-2">
        <div className="mb-2"><AIProviderSelect value={provider} onChange={setProvider} disabled={isLoading} compact /></div>
        <Card className="flex items-end gap-2 p-2 bg-secondary/5 border-border/50">
          <Button
            variant="ghost"
            size="icon"
            className="shrink-0 w-9 h-9 text-destructive hover:text-destructive hover:bg-destructive/10"
            onClick={onClearCanvas}
            disabled={isLoading}
            title={t.clearCanvas}
          >
            <Trash2 className="w-4 h-4" />
          </Button>

          <Textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            onCompositionStart={() => setIsComposing(true)}
            onCompositionEnd={() => setIsComposing(false)}
            placeholder={t.placeholder}
            className="min-h-[40px] max-h-[80px] resize-none border-0 bg-transparent focus-visible:ring-0 p-2 text-base"
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
  )
}
