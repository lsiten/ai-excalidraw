import { useEffect, useRef, useState } from 'react'
import { Play, ChevronDown, Crop, Scan } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { presentations } from '@/lib/presentation/controller'
import { workspace } from '@/lib/workspace/store'
import { message, type CanvasFile } from '@/lib/workspace/types'

export function PresentationActions({
  active,
  projectId,
  disabled,
}: {
  readonly active?: CanvasFile
  readonly projectId?: string
  readonly disabled: boolean
}) {
  const [open, setOpen] = useState(false)
  const [notice, setNotice] = useState<string | null>(null)
  const root = useRef<HTMLDivElement>(null)
  const trigger = useRef<HTMLButtonElement>(null)
  const run = (action: () => Promise<unknown>) => {
    setOpen(false)
    void action().catch((error) => workspace.set({ error: message(error) }))
  }
  useEffect(() => {
    if (!open) return
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false)
    }
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false)
        trigger.current?.focus()
      }
    }
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', key)
    return () => {
      document.removeEventListener('pointerdown', outside)
      document.removeEventListener('keydown', key)
    }
  }, [open])
  if (!projectId) return null
  return (
    <div ref={root} className="relative flex shrink-0">
      <Button
        variant="ghost"
        size="sm"
        disabled={disabled}
        title="播放当前画布所在文件夹"
        onClick={() =>
          run(() =>
            presentations.prepare(projectId, active?.path.split('/').slice(0, -1).join('/') ?? ''),
          )
        }
      >
        <Play size={14} />
        <span className="ml-1">播放</span>
      </Button>
      <Button
        ref={trigger}
        variant="ghost"
        size="icon"
        className="h-8 w-6"
        disabled={disabled || !active}
        aria-label="播放区域设置"
        title="播放区域设置"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <ChevronDown size={13} />
      </Button>
      {open && active && (
        <div className="absolute right-0 top-full z-40 mt-1 w-56 rounded-lg border border-border bg-card p-1 shadow-xl">
          <button
            className="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs hover:bg-muted"
            onClick={() =>
              run(async () => {
                await presentations.selection()
                setNotice('已将当前选区设为播放区域')
              })
            }
          >
            <Crop size={14} />
            将选区设为播放区域
          </button>
          <button
            className="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs hover:bg-muted"
            onClick={() =>
              run(async () => {
                await presentations.setArea(active.projectId, active.path, null)
                setNotice('已恢复播放整张画布')
              })
            }
          >
            <Scan size={14} />
            恢复播放整张画布
          </button>
          <p className="px-3 py-2 text-[11px] leading-5 text-muted-foreground">
            先框选课件主体，再设置区域。原始画布内容保持不变。
          </p>
        </div>
      )}
      {notice && (
        <div
          role="status"
          className="absolute right-0 top-full z-30 mt-1 flex w-56 items-center gap-2 rounded-lg border border-border bg-card p-3 text-xs shadow-lg"
        >
          <span className="flex-1">{notice}</span>
          <button aria-label="关闭播放区域提示" onClick={() => setNotice(null)}>
            ×
          </button>
        </div>
      )}
    </div>
  )
}
