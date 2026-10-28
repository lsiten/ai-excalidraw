import { useEffect, useRef, useState, type ReactNode, type PointerEvent, type KeyboardEvent } from 'react'
import { z } from 'zod'
import { readStorage, writeStorage } from '@/lib/storage'

const WIDTH_KEY = 'ai-excalidraw-explorer-width'
const DEFAULT_WIDTH = 244
const MIN_WIDTH = 220
const MAX_WIDTH = 480

export function ExplorerPane({ children, narrow }: { readonly children: ReactNode; readonly narrow: boolean }) {
  const paneRef = useRef<HTMLDivElement>(null)
  const drag = useRef<{ readonly pointerId: number; readonly startX: number; readonly startWidth: number; readonly startPreferredWidth: number; width: number } | null>(null)
  const [preferredWidth, setPreferredWidth] = useState(() => {
    const saved = z.number().finite().min(MIN_WIDTH).max(MAX_WIDTH).safeParse(readStorage<unknown>(WIDTH_KEY, DEFAULT_WIDTH))
    return saved.success ? saved.data : DEFAULT_WIDTH
  })
  const [availableWidth, setAvailableWidth] = useState(() => window.innerWidth)
  const [dragging, setDragging] = useState(false)
  const maxWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, availableWidth - (narrow ? 48 : 640)))
  const clamp = (value: number) => Math.round(Math.max(MIN_WIDTH, Math.min(maxWidth, value)))
  const width = clamp(preferredWidth)

  useEffect(() => {
    const parent = paneRef.current?.parentElement
    if (!parent) return
    const observer = new ResizeObserver(([entry]) => setAvailableWidth(entry.contentRect.width))
    observer.observe(parent)
    return () => observer.disconnect()
  }, [])

  useEffect(() => {
    if (!dragging) return
    const { cursor, userSelect } = document.body.style
    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
    return () => {
      document.body.style.cursor = cursor
      document.body.style.userSelect = userSelect
    }
  }, [dragging])

  const save = (value: number) => {
    const next = clamp(value)
    setPreferredWidth(next)
    writeStorage(WIDTH_KEY, next)
  }
  const stop = (event: PointerEvent<HTMLDivElement>, cancelled: boolean) => {
    const current = drag.current
    if (!current || current.pointerId !== event.pointerId) return
    drag.current = null
    setDragging(false)
    if (cancelled || current.width === current.startWidth) setPreferredWidth(current.startPreferredWidth)
    else save(current.width)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
  }
  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = event.shiftKey ? 40 : 10
    let next: number
    switch (event.key) {
      case 'ArrowLeft': next = width - step; break
      case 'ArrowRight': next = width + step; break
      case 'Home': next = MIN_WIDTH; break
      case 'End': next = maxWidth; break
      default: return
    }
    event.preventDefault()
    if (clamp(next) !== width) save(next)
  }

  return <div ref={paneRef} style={{ width }} className={narrow
    ? 'absolute inset-y-0 left-0 z-40 flex shadow-xl'
    : 'relative flex shrink-0'}>
    <div id="workspace-explorer-pane" className="flex min-w-0 flex-1">{children}</div>
    <div
      role="separator" tabIndex={0} aria-label="调整文件栏宽度" aria-orientation="vertical"
      aria-controls="workspace-explorer-pane" aria-valuemin={MIN_WIDTH} aria-valuemax={maxWidth} aria-valuenow={width}
      title="拖拽调整文件栏宽度，双击恢复默认；方向键微调"
      className="absolute inset-y-0 -right-1 z-50 w-2 cursor-col-resize touch-none hover:bg-border/70 focus-visible:bg-border focus-visible:outline-2 focus-visible:outline-ring"
      onPointerDown={(event) => {
        if (!event.isPrimary || event.button !== 0) return
        event.preventDefault()
        event.currentTarget.focus()
        event.currentTarget.setPointerCapture(event.pointerId)
        drag.current = { pointerId: event.pointerId, startX: event.clientX, startWidth: width, startPreferredWidth: preferredWidth, width }
        setDragging(true)
      }}
      onPointerMove={(event) => {
        const current = drag.current
        if (!current || current.pointerId !== event.pointerId) return
        current.width = clamp(current.startWidth + event.clientX - current.startX)
        setPreferredWidth(current.width)
      }}
      onPointerUp={(event) => stop(event, false)}
      onPointerCancel={(event) => stop(event, true)}
      onLostPointerCapture={(event) => stop(event, true)}
      onDoubleClick={() => save(DEFAULT_WIDTH)}
      onKeyDown={onKeyDown}
    />
  </div>
}
