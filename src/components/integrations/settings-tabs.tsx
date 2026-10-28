import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

const tabs = [
  { id: 'model', label: 'AI 模型' },
  { id: 'integrations', label: '安装 AI 工具' },
] as const
export type SettingsTab = (typeof tabs)[number]['id']

type SettingsTabsProps = {
  readonly id: string
  readonly value: SettingsTab
  readonly onChange: (tab: SettingsTab) => void
}

export function SettingsTabs({ id, value, onChange }: SettingsTabsProps) {
  return (
    <div
      role="tablist"
      aria-label="设置分类"
      className="mx-6 mb-2 flex shrink-0 gap-1 rounded-lg bg-muted p-1"
    >
      {tabs.map((tab, index) => (
        <Button
          key={tab.id}
          id={`${id}-${tab.id}-tab`}
          type="button"
          role="tab"
          variant="ghost"
          size="sm"
          aria-selected={value === tab.id}
          aria-controls={`${id}-${tab.id}-panel`}
          tabIndex={value === tab.id ? 0 : -1}
          className={cn('flex-1', value === tab.id && 'bg-card text-foreground shadow-sm')}
          onClick={() => onChange(tab.id)}
          onKeyDown={(event) => {
            const next =
              event.key === 'Home'
                ? tabs[0]
                : event.key === 'End'
                  ? tabs[1]
                  : event.key === 'ArrowRight' || event.key === 'ArrowLeft'
                    ? tabs[1 - index]
                    : undefined
            if (!next) return
            event.preventDefault()
            onChange(next.id)
            document.getElementById(`${id}-${next.id}-tab`)?.focus()
          }}
        >
          {tab.label}
        </Button>
      ))}
    </div>
  )
}
