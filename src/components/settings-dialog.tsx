import { useState, useRef, useId } from 'react'
import { isTauri } from '@tauri-apps/api/core'
import * as Dialog from '@radix-ui/react-dialog'
import { Settings, X, Check, Download, Upload } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'
import { getAIConfig, saveAIConfig, isConfigValid, type AIConfig } from '@/lib/ai'
import { useLocale } from '@/hooks/use-locale'
import { downloadBackup, importBackupFromFile } from '@/lib/backup'
import { useToast } from '@/components/ui/toast'
import type { Locale } from '@/lib/i18n'
import { IntegrationsPanel } from '@/components/integrations/integrations-panel'
import { AIProviderSelect } from '@/components/excalidraw/ai-provider-select'
import { SettingsTabs, type SettingsTab } from '@/components/integrations/settings-tabs'

interface SettingsDialogProps {
  projectId?: string
  projectDirectory?: string
  onConfigChange?: (config: AIConfig) => void
}

export function SettingsDialog({ onConfigChange, projectId, projectDirectory }: SettingsDialogProps) {
  const { t, locale, setLocale } = useLocale()
  const { showToast } = useToast()
  const fileInputRef = useRef<HTMLInputElement>(null)
  const [config, setConfig] = useState<AIConfig>(() => getAIConfig())
  const [open, setOpen] = useState(() => !isTauri() && !isConfigValid(getAIConfig()))
  const [saved, setSaved] = useState(false)
  const [tab, setTab] = useState<SettingsTab>('model')
  const [installing, setInstalling] = useState(false)
  const installingRef = useRef(false)
  const tabsId = useId()
  const desktop = isTauri()

  const handleOpenChange = (next: boolean) => {
    if (!next && installing) return
    if (next) setConfig(getAIConfig())
    setOpen(next)
  }

  const handleSave = () => {
    if (!saveAIConfig(config)) {
      showToast(t.saveConfigFailed, 3000)
      return
    }
    onConfigChange?.(config)
    setSaved(true)
    setTimeout(() => {
      setSaved(false)
      if (!installingRef.current) setOpen(false)
    }, 1000)
  }

  const handleImport = async (file: File | undefined) => {
    if (!file) return
    const result = await importBackupFromFile(file)
    if (result.ok) {
      showToast(t.importSuccess, 2000)
      setTimeout(() => window.location.reload(), 500)
    } else {
      showToast(t.importFailed, 3000)
    }
  }

  const isValid = isConfigValid(config)

  return (
    <Dialog.Root open={open} onOpenChange={handleOpenChange}>
      <Dialog.Trigger asChild>
        <Button variant="ghost" size="icon" className="w-8 h-8" title={desktop ? '设置' : t.settings} aria-label={desktop ? '设置' : t.settings}>
          <Settings className="w-4 h-4" />
        </Button>
      </Dialog.Trigger>

      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 animate-in fade-in" />
        <Dialog.Content
          className={cn(
            'fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-50 flex max-h-[90dvh] w-[calc(100%_-_2rem)] flex-col overflow-hidden rounded-xl bg-card border border-border shadow-xl animate-in fade-in',
            desktop ? 'max-w-[640px]' : 'max-w-md',
          )}
        >
          <div className="flex shrink-0 items-center justify-between px-6 pt-6 pb-4">
            <Dialog.Title className="text-lg font-semibold">{desktop ? '设置' : t.settings}</Dialog.Title>
            <Dialog.Close asChild>
              <Button
                variant="ghost"
                size="icon"
                className="w-8 h-8"
                aria-label="关闭设置"
                disabled={installing}
              >
                <X className="w-4 h-4" />
              </Button>
            </Dialog.Close>
          </div>
          <Dialog.Description className="sr-only">
            配置 AI 模型或安装本机 AI 工具接入。
          </Dialog.Description>
          {desktop && <SettingsTabs id={tabsId} value={tab} onChange={setTab} />}

          <div
            id={`${tabsId}-model-panel`}
            role={desktop ? 'tabpanel' : undefined}
            aria-labelledby={desktop ? `${tabsId}-model-tab` : undefined}
            className={cn('min-h-0 flex-1 flex-col', tab === 'model' ? 'flex' : 'hidden')}
          >
            <div className="space-y-4 min-h-0 overflow-y-auto px-6 py-4">
              <AIProviderSelect value={config.provider} onChange={(provider) => setConfig({ ...config, provider })} />
              {config.provider === 'api' && <>
              <div className="space-y-2">
                <label htmlFor={`${tabsId}-api-key`} className="text-sm font-medium">
                  {t.apiKey}
                </label>
                <Input
                  id={`${tabsId}-api-key`}
                  type="password"
                  value={config.apiKey}
                  onChange={(e) => setConfig({ ...config, apiKey: e.target.value })}
                  placeholder="sk-..."
                />
              </div>

              <div className="space-y-2">
                <label htmlFor={`${tabsId}-base-url`} className="text-sm font-medium">
                  {t.baseURL}
                </label>
                <Input
                  id={`${tabsId}-base-url`}
                  value={config.baseURL}
                  onChange={(e) => setConfig({ ...config, baseURL: e.target.value })}
                  placeholder="https://api.openai.com/v1"
                />
                <p className="text-xs text-muted-foreground">{t.baseURLHint}</p>
              </div>

              <div className="space-y-2">
                <label htmlFor={`${tabsId}-model`} className="text-sm font-medium">
                  {t.model}
                </label>
                <Input
                  id={`${tabsId}-model`}
                  value={config.model}
                  onChange={(e) => setConfig({ ...config, model: e.target.value })}
                  placeholder="gpt-4o"
                />
              </div>

              </>}
              <div className="space-y-2">
                <label htmlFor={`${tabsId}-language`} className="text-sm font-medium">
                  {t.language}
                </label>
                <select
                  id={`${tabsId}-language`}
                  className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm"
                  value={locale}
                  onChange={(e) => setLocale(e.target.value as Locale)}
                >
                  <option value="zh-CN">中文</option>
                  <option value="en">English</option>
                </select>
              </div>

              <div className="space-y-2 pt-2 border-t border-border">
                <div className="flex gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    className="flex-1 gap-1.5"
                    onClick={() => downloadBackup()}
                  >
                    <Download className="w-4 h-4" />
                    {t.exportData}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    className="flex-1 gap-1.5"
                    onClick={() => fileInputRef.current?.click()}
                  >
                    <Upload className="w-4 h-4" />
                    {t.importData}
                  </Button>
                  <input
                    ref={fileInputRef}
                    type="file"
                    accept="application/json,.json"
                    className="hidden"
                    onChange={(e) => {
                      void handleImport(e.target.files?.[0])
                      e.target.value = ''
                    }}
                  />
                </div>
              </div>
              {desktop && (
                <Button
                  type="button"
                  variant="outline"
                  className="w-full"
                  onClick={() => {
                    setTab('integrations')
                    document.getElementById(`${tabsId}-integrations-tab`)?.focus()
                  }}
                >
                  安装 AI 工具
                </Button>
              )}
            </div>

            <div className="flex shrink-0 justify-end gap-2 border-t border-border px-6 py-4">
              <Dialog.Close asChild>
                <Button variant="outline" disabled={installing}>
                  {t.cancel}
                </Button>
              </Dialog.Close>
              <Button
                onClick={handleSave}
                disabled={!isValid || installing || saved}
                className={cn(saved && 'bg-green-600 hover:bg-green-600')}
              >
                {saved ? (
                  <>
                    <Check className="w-4 h-4 mr-1" />
                    {t.saved}
                  </>
                ) : (
                  t.save
                )}
              </Button>
            </div>
          </div>
          {desktop && (
            <div
              id={`${tabsId}-integrations-panel`}
              role="tabpanel"
              aria-labelledby={`${tabsId}-integrations-tab`}
              className={cn('min-h-0 flex-1 flex-col', tab === 'integrations' ? 'flex' : 'hidden')}
            >
              <IntegrationsPanel
                key={projectId ?? 'global'}
                projectId={projectId}
                projectDirectory={projectDirectory}
                onInstallingChange={(busy) => {
                  installingRef.current = busy
                  setInstalling(busy)
                }}
              />
            </div>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
