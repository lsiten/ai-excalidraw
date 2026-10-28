import { useEffect, useId, useRef, useState } from 'react'
import { History, MessageSquare, PanelRightClose, Plus, Sparkles, Trash2 } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import { useLocale } from '@/hooks/use-locale'
import type { useChatHistory } from './use-chat-history'
import type { ExcalidrawWrapperRef } from './wrapper'

interface ChatPanelHeaderProps {
  chatHistory: ReturnType<typeof useChatHistory>
  excalidrawRef?: React.RefObject<ExcalidrawWrapperRef | null>
  onNewChat: () => void
  onClose?: () => void
}

export function ChatPanelHeader({
  chatHistory,
  excalidrawRef,
  onNewChat,
  onClose,
}: ChatPanelHeaderProps) {
  const { t } = useLocale()
  const [open, setOpen] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const history = useRef<HTMLDivElement>(null)
  const historyId = useId()
  const { sessions, currentSessionId, switchSession, deleteSession } = chatHistory
  const close = () => {
    setOpen(false)
    trigger.current?.focus()
  }

  useEffect(() => {
    if (!open) return
    history.current?.focus()
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false)
    }
    const escape = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      event.preventDefault()
      event.stopPropagation()
      setOpen(false)
      trigger.current?.focus()
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape, true)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', escape, true)
    }
  }, [open])

  return (
    <div ref={root} className="relative z-20 shrink-0">
      <div className="flex items-center gap-1 border-b border-border bg-secondary/5 px-2 py-2">
        <Button
          ref={trigger}
          variant="ghost"
          size="icon"
          className="h-8 w-8 shrink-0"
          title={t.chatHistory}
          aria-label={t.chatHistory}
          aria-expanded={open}
          aria-controls={historyId}
          onClick={() => setOpen(!open)}
        >
          <History className="h-4 w-4" />
        </Button>
        <div className="flex min-w-0 items-center gap-1.5 text-sm font-medium">
          <Sparkles className="h-4 w-4 shrink-0 text-primary" />
          <span className="truncate">{t.appTitle}</span>
        </div>
        <Button
          variant="ghost"
          size="sm"
          className="ml-auto h-8 shrink-0 gap-1 px-2 text-xs"
          onClick={() => {
            onNewChat()
            setOpen(false)
          }}
        >
          <Plus className="h-3.5 w-3.5" />
          {t.newChat}
        </Button>
        {onClose && (
          <Button
            variant="ghost"
            size="icon"
            className="h-8 w-8 shrink-0"
            onClick={onClose}
            title={t.closeAIPanel}
            aria-label={t.closeAIPanel}
          >
            <PanelRightClose className="h-4 w-4" />
          </Button>
        )}
      </div>
      {open && (
        <div
          ref={history}
          id={historyId}
          role="region"
          aria-label={t.chatHistory}
          tabIndex={-1}
          className="absolute left-2 right-2 top-full mt-1 max-h-[min(360px,60vh)] overflow-y-auto rounded-xl border border-border bg-card p-2 shadow-xl"
        >
          <p className="px-2 py-1.5 text-xs font-medium text-muted-foreground">{t.chatHistory}</p>
          {sessions.length === 0 && (
            <p className="p-3 text-sm text-muted-foreground">{t.emptyChatHistory}</p>
          )}
          {sessions.map((session) => (
            <div
              key={session.id}
              className={cn(
                'group flex min-w-0 items-center rounded-lg',
                session.id === currentSessionId
                  ? 'bg-primary/10 text-primary'
                  : 'hover:bg-secondary/50',
              )}
            >
              <button
                type="button"
                aria-current={session.id === currentSessionId ? 'true' : undefined}
                className="flex min-w-0 flex-1 items-center gap-2 rounded-lg px-2 py-2 text-left text-sm"
                onClick={() => {
                  switchSession(session.id)
                  excalidrawRef?.current?.switchToSession(
                    session.id,
                    session.useIndependentCanvas ?? false,
                  )
                  close()
                }}
              >
                <MessageSquare className="h-4 w-4 shrink-0" />
                <span className="truncate">{session.title}</span>
              </button>
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 shrink-0"
                aria-label={`${t.deleteChat}: ${session.title}`}
                title={t.deleteChat}
                onClick={() => deleteSession(session.id)}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
