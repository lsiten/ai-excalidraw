import * as Dialog from '@radix-ui/react-dialog'
import { useState } from 'react'
import { Play, X, Save, Loader2, Folder } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { orderedPages, type Deck } from '@/lib/presentation/types'
import { presentations } from '@/lib/presentation/controller'
import { message } from '@/lib/workspace/types'
import { DeckPages } from './deck-pages'
import { SlidePreview } from './slide-preview'

export function DeckDialog({
  deck,
  loading,
  error,
}: {
  readonly deck: Deck
  readonly loading: string | null
  readonly error: string | null
}) {
  const [saving, setSaving] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const [saveError, setSaveError] = useState<string | null>(null)
  const pages = orderedPages(deck),
    selected = pages.includes(deck.selected ?? '') ? deck.selected : pages[0]
  const disabled = !!loading || saving
  const update = (next: Deck) => {
    setNotice(null)
    presentations.update(next)
  }
  const save = async () => {
    setSaving(true)
    setSaveError(null)
    try {
      await presentations.save(deck)
      setNotice('演示顺序已保存')
    } catch (error) {
      setSaveError(message(error))
    } finally {
      setSaving(false)
    }
  }
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open && !disabled) void presentations.stop()
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-50 flex h-[min(760px,90dvh)] w-[min(1060px,94vw)] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl"
          onEscapeKeyDown={(event) => {
            if (disabled) event.preventDefault()
          }}
          onInteractOutside={(event) => event.preventDefault()}
        >
          <header className="flex shrink-0 items-start gap-3 border-b border-border p-5">
            <div className="min-w-0 flex-1">
              <Dialog.Title className="text-lg font-semibold">文件夹演示</Dialog.Title>
              <Dialog.Description className="mt-1 flex items-center gap-1.5 text-xs text-muted-foreground">
                <Folder size={13} />
                <span className="truncate" title={deck.folder}>
                  {deck.folder || '项目根目录'}
                </span>{' '}
                · {pages.length} 页
              </Dialog.Description>
            </div>
            <Button
              variant="ghost"
              size="icon"
              disabled={disabled}
              onClick={() => void presentations.stop()}
              aria-label="关闭演示准备"
            >
              <X size={18} />
            </Button>
          </header>
          <div className="flex min-h-0 flex-1">
            <DeckPages
              deck={{ ...deck, selected: selected ?? null }}
              disabled={disabled}
              onChange={update}
            />
            <section className="flex min-w-0 flex-1 flex-col p-5" aria-label="页面预览">
              <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
                <p className="text-sm font-medium">
                  {selected ? `第 ${pages.indexOf(selected) + 1} 页预览` : '暂无页面'}
                </p>
                <label className="flex items-center gap-2 text-xs">
                  <input
                    type="checkbox"
                    checked={deck.settings.includeSubfolders}
                    disabled={disabled}
                    onChange={(event) =>
                      update({
                        ...deck,
                        settings: { ...deck.settings, includeSubfolders: event.target.checked },
                      })
                    }
                  />
                  包含子文件夹
                </label>
              </div>
              <div className="flex min-h-0 flex-1 items-center justify-center rounded-lg bg-muted/50 p-3">
                {selected ? (
                  <SlidePreview
                    key={selected}
                    projectId={deck.projectId}
                    path={selected}
                    area={deck.data.areas[selected]}
                    className="h-full w-full"
                  />
                ) : (
                  <p className="text-sm text-muted-foreground">选择有画布的文件夹开始演示</p>
                )}
              </div>
              <p className="mt-4 truncate text-xs text-muted-foreground" title={selected ?? ''}>
                {selected?.replace(/\.excalidraw$/, '')}
              </p>
              <p className="mt-2 text-xs leading-6 text-muted-foreground">
                拖动左侧页面或使用上下箭头排序。未手动排序的页面按文件名自然排序。演示中可临时批注，翻页保留，退出后全部清空。
              </p>
            </section>
          </div>
          <footer className="flex shrink-0 flex-wrap items-center gap-3 border-t border-border p-4">
            <p
              className={`min-w-0 flex-1 text-xs ${error || saveError ? 'text-destructive' : 'text-muted-foreground'}`}
              role={error || saveError ? 'alert' : 'status'}
            >
              {loading || error || saveError || notice || '顺序调整将在保存或开始播放时记录。'}
            </p>
            <Button
              variant="outline"
              size="sm"
              disabled={disabled || !pages.length}
              onClick={() => void save()}
            >
              <Save size={14} className="mr-1" />
              保存顺序
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={disabled || !pages.length}
              onClick={() => void presentations.start(deck, null)}
            >
              从头播放
            </Button>
            <Button
              size="sm"
              disabled={disabled || !pages.length}
              onClick={() => void presentations.start(deck, selected ?? null)}
            >
              {loading ? (
                <Loader2 size={14} className="mr-1 animate-spin" />
              ) : (
                <Play size={14} className="mr-1" />
              )}
              从第 {selected ? pages.indexOf(selected) + 1 : 1} 页播放
            </Button>
          </footer>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
