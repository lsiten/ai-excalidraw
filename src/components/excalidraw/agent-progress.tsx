import { useId, useState } from 'react'
import { Brain, Check, ChevronDown, CircleAlert, FilePenLine, ListChecks, Loader2, Search, Terminal, Wrench } from 'lucide-react'
import { useLocale } from '@/hooks/use-locale'
import type { LocalReplyPart, LocalChatActivity } from '@/lib/local-chat'
import { cn } from '@/lib/utils'
import { removeJsonObjects } from '@/lib/message-content'
import type { Messages } from '@/lib/i18n'
import type { AgentRunState } from '@/lib/agent/protocol'

type ToolActivity = Extract<LocalChatActivity, { type: 'tool' }>

export function AgentProgress({ parts, isStreaming, runState }: { readonly parts: readonly LocalReplyPart[]; readonly isStreaming: boolean; readonly runState?: AgentRunState }) {
  const { t } = useLocale()
  const [expanded, setExpanded] = useState<boolean | null>(null)
  const id = useId()
  const tools = parts.filter((part): part is ToolActivity => part.type === 'tool')
  const failed = runState ? ['failed','interrupted','needs_reconciliation','cancelled'].includes(runState) : tools.some((tool) => tool.status === 'failed' || (!isStreaming && tool.status === 'running')) || parts.some((part) => part.type === 'status' && part.level === 'error')
  const error = parts.filter((part) => part.type === 'status' && part.level === 'error').at(-1)
  const isExpanded = expanded ?? (isStreaming || failed)
  const last = parts.at(-1)
  const stepCount = parts.length
  const hasDetails = parts.some((part) => part.type === 'thinking' || part.type === 'tool')
  const status = isStreaming
    ? last?.type === 'status' ? last.message : last?.type === 'thinking' ? t.thinkingNow : last?.type === 'tool' ? toolLabel(last.name, t) : t.agentWorking
    : failed ? t.toolFailed : stepCount > 0 ? t.processSteps(stepCount) : t.toolCompleted

  return <div className="min-w-0 rounded-lg border border-border bg-background/60 text-xs">
    <button type="button" aria-expanded={isExpanded} aria-controls={id}
      onClick={() => setExpanded(!isExpanded)}
      className="flex min-h-11 w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-muted-foreground hover:bg-muted/60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
      {isStreaming ? <Loader2 aria-hidden className="h-3.5 w-3.5 shrink-0 animate-spin motion-reduce:animate-none" />
        : failed ? <CircleAlert aria-hidden className="h-3.5 w-3.5 shrink-0 text-destructive-ink" />
          : <ListChecks aria-hidden className="h-3.5 w-3.5 shrink-0" />}
      <span className="shrink-0 font-medium">{t.executionProcess}</span>
      <span className="min-w-0 flex-1 truncate text-right" role="status">{status}</span>
      <ChevronDown aria-hidden className={cn('h-3.5 w-3.5 shrink-0', isExpanded && 'rotate-180')} />
    </button>
    {!isExpanded && error?.type === 'status' && <p className="px-3 pb-3 whitespace-pre-wrap break-words text-balance text-destructive-ink">{error.message}</p>}
    {isExpanded && <div id={id} className="space-y-3 border-t border-border p-3">
      {parts.length === 0 && <p className="text-muted-foreground">{t.agentStarting}</p>}
      {parts.map((part, index) => {
        switch (part.type) {
          case 'tool': return <ToolStep key={part.id} activity={part} isStreaming={isStreaming} />
          case 'thinking': return <div key={index} className="space-y-2 text-muted-foreground">
            <p className="flex items-center gap-2 font-medium"><Brain aria-hidden className="h-3.5 w-3.5 shrink-0" />{t.thinking}</p>
            <div className="max-h-48 overflow-auto whitespace-pre-wrap break-words text-balance leading-relaxed">{part.text}</div>
          </div>
          case 'chunk': return <p key={index} className="whitespace-pre-wrap break-words leading-relaxed text-foreground">{removeJsonObjects(part.text) || t.generated}</p>
          case 'status': return <p key={index} className={cn('flex items-start gap-2 text-muted-foreground', part.level === 'error' && 'text-destructive-ink')}>
            {part.level === 'error' ? <CircleAlert aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0" />
              : isStreaming && index === parts.length - 1 ? <Loader2 aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0 animate-spin motion-reduce:animate-none" />
                : <Check aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0" />}
            <span className="min-w-0 whitespace-pre-wrap break-words text-balance leading-relaxed">{part.message}</span>
          </p>
        }
      })}
      {!isStreaming && !hasDetails && !failed && <p className="text-balance leading-relaxed text-muted-foreground">{t.agentNoDetails}</p>}
    </div>}
  </div>
}

function ToolStep({ activity, isStreaming }: { readonly activity: ToolActivity; readonly isStreaming: boolean }) {
  const { t } = useLocale()
  const Icon = activity.name === 'command_execution' || activity.name === 'Bash' || activity.name === 'shell' ? Terminal
    : activity.name === 'file_change' ? FilePenLine : activity.name === 'web_search' ? Search
      : activity.name === 'todo_list' || activity.name === 'plan' ? ListChecks : Wrench
  const status = activity.status === 'running' && !isStreaming ? 'failed' : activity.status
  const label = toolLabel(activity.name, t)
  return <details className="group/tool min-w-0 rounded-lg bg-muted/60">
    <summary className="flex min-h-11 cursor-pointer list-none items-start gap-2 rounded-lg px-2 py-2.5 text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring [&::-webkit-details-marker]:hidden">
      <Icon aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0" />
      <span className="min-w-0 flex-1 break-words font-medium">{label}</span>
      <span className={cn('flex shrink-0 items-center gap-1', status === 'failed' && 'text-destructive-ink')}>
        {status === 'running' ? <Loader2 aria-hidden className="h-3 w-3 animate-spin motion-reduce:animate-none" />
          : status === 'failed' ? <CircleAlert aria-hidden className="h-3 w-3" /> : <Check aria-hidden className="h-3 w-3" />}
        {{ running: t.toolRunning, completed: t.toolCompleted, failed: t.toolFailed }[status]}
      </span>
      <ChevronDown aria-hidden className="mt-0.5 h-3.5 w-3.5 shrink-0 group-open/tool:rotate-180" />
    </summary>
    <div className="max-h-48 space-y-3 overflow-auto border-t border-border p-2 text-muted-foreground">
      {label !== activity.name && <p className="break-all font-mono">{activity.name}</p>}
      {activity.input && <div><p className="mb-1 font-medium">{t.toolInput}</p><pre className="whitespace-pre-wrap break-all font-mono">{formatPayload(activity.input)}</pre></div>}
      {activity.output ? <div><p className="mb-1 font-medium">{t.toolOutput}</p><pre className="whitespace-pre-wrap break-all font-mono">{formatPayload(activity.output)}</pre></div>
        : <p>{status === 'running' ? t.toolWaiting : activity.status === 'running' ? t.toolIncomplete : t.toolCompleted}</p>}
    </div>
  </details>
}

function toolLabel(name: string, t: Messages): string {
  const action = name.split(/:|__/).at(-1) ?? name
  const labels: Readonly<Record<string, string>> = {
    command_execution: t.commandExecution, shell: t.commandExecution, Bash: t.commandExecution,
    file_change: t.fileChange, fileChange: t.fileChange, web_search: t.webSearch, webSearch: t.webSearch,
    todo_list: t.agentPlan, plan: t.agentPlan, update_plan: t.agentPlan, commandExecution: t.commandExecution,
    mcp_tool_call: t.mcpTool, get_canvas: t.readCanvas, get_canvas_elements: t.readCanvas,
    get_workspace: t.readWorkspace, add_elements: t.addElements, update_elements: t.updateElements,
    delete_elements: t.deleteElements, fit_canvas: t.fitCanvas,
  }
  return labels[action] ?? name
}

function formatPayload(text: string): string {
  try {
    const value: unknown = JSON.parse(text)
    return JSON.stringify(value, null, 2)
  } catch (error) {
    if (error instanceof SyntaxError) return text
    throw error
  }
}
