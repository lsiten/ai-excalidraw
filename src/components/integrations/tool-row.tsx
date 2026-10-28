import { useId } from 'react'
import { Input } from '@/components/ui/input'
import { integrationStatusLabels, type IntegrationTool } from '@/lib/integrations'
import { cn } from '@/lib/utils'

type ToolRowProps = {
  readonly tool: IntegrationTool
  readonly selected: boolean
  readonly busy: boolean
  readonly onSelect: (selected: boolean) => void
}

export function IntegrationToolRow({ tool, selected, busy, onSelect }: ToolRowProps) {
  const id = useId()
  return (
    <li className={cn('rounded-lg border border-border', selected && 'bg-muted')}>
      <label htmlFor={id} className="flex cursor-pointer items-start gap-3 p-3">
        <Input
          id={id}
          type="checkbox"
          checked={selected}
          disabled={busy}
          onChange={(event) => onSelect(event.target.checked)}
          aria-describedby={`${id}-detection`}
          className="mt-1 h-4 w-4 shrink-0 cursor-pointer accent-foreground"
        />
        <span className="min-w-0 flex-1 space-y-2">
          <span className="flex flex-wrap items-center justify-between gap-2">
            <span className="text-sm font-medium">{tool.name}</span>
            <span id={`${id}-detection`} className="text-xs text-muted-foreground">
              {tool.detected ? '已检测到' : '未检测到 · 可提前配置'}
            </span>
          </span>
          <span className="flex flex-wrap gap-2">
            {(['mcpStatus', 'skillStatus'] as const).map((key) => (
              <span
                key={key}
                className={cn(
                  'rounded-md bg-background px-2 py-1 text-xs',
                  tool[key] === 'blocked' && 'text-destructive',
                )}
              >
                {key === 'mcpStatus' ? 'MCP' : 'Skill'} · {integrationStatusLabels[tool[key]]}
              </span>
            ))}
          </span>
        </span>
      </label>
      <div className="px-3 pb-3 text-xs text-muted-foreground">
        {tool.note && <p className="mb-2 break-words">{tool.note}</p>}
        <details>
          <summary className="w-fit cursor-pointer rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
            查看 {tool.name} 配置路径
          </summary>
          <dl className="mt-2 space-y-2">
            <div>
              <dt>MCP 配置</dt>
              <dd className="select-text break-all font-mono">
                {tool.configPath || '路径暂不可用'}
              </dd>
            </div>
            <div>
              <dt>Skill 目录</dt>
              <dd className="select-text break-all font-mono">
                {tool.skillPath || '路径暂不可用'}
              </dd>
            </div>
          </dl>
        </details>
      </div>
    </li>
  )
}
