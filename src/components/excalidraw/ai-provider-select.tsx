import { useEffect, useId, useState } from 'react'
import { isTauri } from '@tauri-apps/api/core'
import { RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { aiProviderSchema, type AIProvider } from '@/lib/ai-config'
import { integrationNames, integrationErrorMessage } from '@/lib/integrations'
import { getLocalChatTools, type LocalChatTool } from '@/lib/local-chat'
import { useLocale } from '@/hooks/use-locale'
import { cn } from '@/lib/utils'

export function AIProviderSelect({ value, onChange, disabled, compact = false }: {
  readonly value: AIProvider
  readonly onChange: (provider: AIProvider) => void
  readonly disabled?: boolean
  readonly compact?: boolean
}) {
  const { t } = useLocale()
  const id = useId()
  const desktop = isTauri()
  const [tools, setTools] = useState<readonly LocalChatTool[]>([])
  const [checking, setChecking] = useState(desktop)
  const [error, setError] = useState('')
  const [revision, setRevision] = useState(0)
  useEffect(() => {
    if (!desktop) return
    let disposed = false
    getLocalChatTools().then((result) => {
      if (!disposed) { setTools(result); setError(''); setChecking(false) }
    }).catch((cause: unknown) => {
      if (!disposed) { setError(integrationErrorMessage(cause)); setChecking(false) }
    })
    return () => { disposed = true }
  }, [desktop, revision])
  const selected = tools.find((tool) => tool.id === value)
  const hint = value === 'api'
    ? t.apiSourceHint
    : checking ? t.checkingTools : selected?.note || (selected?.available ? t.localSourceHint : t.localToolUnavailable)

  return (
    <div className="min-w-0 space-y-2">
      <div className={cn('flex min-w-0 gap-2', compact ? 'items-center' : 'flex-col')}>
        <label htmlFor={id} className="shrink-0 text-sm font-medium">{t.chatSource}</label>
        <div className="flex min-w-0 flex-1 items-center gap-1">
          <select
            id={id}
            value={value}
            disabled={disabled}
            onChange={(event) => onChange(aiProviderSchema.parse(event.target.value))}
            className="h-10 w-full min-w-0 rounded-md border border-input bg-background px-2 text-sm text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50"
          >
            <option value="api">{t.modelAPI}</option>
            {(desktop || value !== 'api') && Object.entries(integrationNames).map(([toolId, name]) => (
              <option key={toolId} value={toolId} disabled={!tools.find((tool) => tool.id === toolId)?.available}>
                {name}{!checking && !tools.find((tool) => tool.id === toolId)?.available ? ` · ${t.unavailable}` : ''}
              </option>
            ))}
          </select>
          {desktop && (
            <Button
              type="button" variant="ghost" size="icon" className="h-10 w-10 shrink-0"
              title={t.refreshTools} aria-label={t.refreshTools} disabled={disabled || checking}
              onClick={() => { setChecking(true); setRevision((current) => current + 1) }}
            >
              <RefreshCw className={cn('h-3.5 w-3.5', checking && 'animate-spin motion-reduce:animate-none')} />
            </Button>
          )}
        </div>
      </div>
      {error && <p role="alert" className="break-words text-xs text-destructive">{error}</p>}
      {(!compact || value !== 'api') && <p className="break-words text-xs text-muted-foreground" aria-live="polite">{hint}</p>}
    </div>
  )
}
