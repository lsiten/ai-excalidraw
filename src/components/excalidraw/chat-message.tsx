import { useState, useSyncExternalStore } from 'react'
import { getRuns, subscribeRuns } from '@/lib/agent/store'
import { replayRun } from '@/lib/agent/replay'
import type { AgentRun } from '@/lib/agent/protocol'
import { RunDetails } from './run-details'
import { Brain, ChevronDown, ChevronUp } from 'lucide-react'
import { cn } from '@/lib/utils'
import { useLocale } from '@/hooks/use-locale'
import { removeJsonObjects, parseThinkingContent } from '@/lib/message-content'
import type { ChatMessage } from './use-chat-history'
import { integrationNames } from '@/lib/integrations'
import { AgentProgress } from './agent-progress'
import type { LocalReplyPart } from '@/lib/local-chat'
import { splitReplyParts } from '@/lib/chat-reply'

export function MessageBubble({ message, isStreaming = false, onContinue }: { readonly message: ChatMessage; readonly isStreaming?: boolean; readonly onContinue?: (run: AgentRun) => void }) {
  const runs = useSyncExternalStore(subscribeRuns, getRuns, getRuns)
  const run = message.runId ? runs.find(run => run.header.id === message.runId) : undefined
  const isUser = message.role === 'user'
  const isLocal = !isUser && (message.runId || (message.source && message.source !== 'api'))
  const parts: readonly LocalReplyPart[] = run && !isStreaming ? replayRun(run) : message.parts ?? [
    ...(message.thinking ? [{ type: 'thinking' as const, text: message.thinking }] : []),
    ...(message.activities ?? []),
  ]
  const split = splitReplyParts(parts)
  const answer = message.parts || run ? split.answer : message.content
  const progress = message.parts || run ? split.progress : parts
  const errors = message.activities?.filter((activity) => activity.type === 'status' && activity.level === 'error') ?? []

  return (
    <div className={cn('flex', isUser ? 'justify-end' : 'justify-start')}>
      <div
        className={cn(
          'min-w-0 max-w-[90%] rounded-2xl px-4 py-2.5 text-sm',
          isUser
            ? 'bg-primary text-primary-foreground rounded-br-md'
            : 'bg-secondary/50 text-foreground rounded-bl-md',
          isLocal && 'w-full',
        )}
      >
        <div className="whitespace-pre-wrap break-words">
          {!isUser && message.source && message.source !== 'api' && <p className="mb-2 text-xs text-muted-foreground">{integrationNames[message.source]}</p>}
          {isLocal ? <div className="space-y-3">
            {(progress.length > 0 || isStreaming) && <AgentProgress parts={progress} isStreaming={isStreaming} runState={run?.header.state} />}
            {run && <RunDetails run={run} onContinue={onContinue} />}
            {answer && <div className="text-balance"><AssistantMessage content={answer} completed={!run || run.header.state==='succeeded'} /></div>}
          </div> : <>
            {!isUser && message.thinking && <ThinkingBlock content={message.thinking} />}
            {isUser ? message.content : <>
              {(message.content || errors.length === 0) && <AssistantMessage content={message.content} />}
              {errors.map((error, index) => error.type === 'status' && <p key={index} className="mt-2 whitespace-pre-wrap break-words text-balance text-destructive-ink">{error.message}</p>)}
            </>}
          </>}
        </div>
      </div>
    </div>
  )
}

function ThinkingBlock({ content }: { content: string }) {
  const { t } = useLocale()
  const [isExpanded, setIsExpanded] = useState(false)

  if (!content) return null

  return (
    <div className="mb-2 rounded-lg bg-muted border border-border overflow-hidden">
      <button
        onClick={() => setIsExpanded(!isExpanded)}
        className="w-full flex items-center gap-2 px-3 py-2 text-xs text-primary/70 hover:bg-primary/10 transition-colors"
        type="button"
        aria-expanded={isExpanded}
      >
        <Brain className="w-3.5 h-3.5" />
        <span className="font-medium">{t.thinking}</span>
        {isExpanded ? (
          <ChevronUp className="w-3.5 h-3.5 ml-auto" />
        ) : (
          <ChevronDown className="w-3.5 h-3.5 ml-auto" />
        )}
      </button>
      {isExpanded && (
        <div className="px-3 py-2 text-xs text-foreground/60 border-t border-primary/10 whitespace-pre-wrap">
          {content}
        </div>
      )}
    </div>
  )
}

function AssistantMessage({ content, completed = true }: { content: string; completed?: boolean }) {
  const { t } = useLocale()
  const { thinking, main } = parseThinkingContent(content)
  const displayContent = removeJsonObjects(main)
  const isThinking = content.includes('<think>') && !content.includes('</think>')

  if (!displayContent && !thinking) {
    const hasElements = /"type"\s*:\s*"(rectangle|ellipse|diamond|text|arrow|line)"/.test(content)
    if (hasElements) {
      return <span className="text-foreground/50 italic">{completed?t.generated:t.toolIncomplete}</span>
    }
    if (isThinking) {
      return (
        <div className="flex items-center gap-2 text-foreground/50 italic">
          <Brain className="w-4 h-4 animate-pulse" />
          <span>{t.thinkingNow}</span>
        </div>
      )
    }
    return <span className="text-foreground/50 italic">{t.generating}</span>
  }

  return (
    <>
      {thinking && <ThinkingBlock content={thinking} />}
      {displayContent || <span className="text-foreground/50 italic">{t.generated}</span>}
    </>
  )
}
