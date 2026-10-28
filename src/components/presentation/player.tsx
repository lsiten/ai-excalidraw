import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { presentations } from '@/lib/presentation/controller'
import type { PresentationState } from '@/lib/presentation/types'
import {
  EMPTY_HISTORY,
  commitInk,
  undoInk,
  redoInk,
  type AnnotationTool,
  type InkHistory,
  type InkStroke,
} from '@/lib/presentation/annotations'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { AnnotationLayer } from './annotation-layer'
import { PlayerToolbar, INK_COLORS } from './player-toolbar'

export function PresentationPlayer({
  state,
}: {
  readonly state: Extract<PresentationState, { phase: 'playing' }>
}) {
  const [tool, setTool] = useState<AnnotationTool>('pointer')
  const [color, setColor] = useState<string>(INK_COLORS[0])
  const [ink, setInk] = useState<Readonly<Record<string, InkHistory>>>({})
  const [thumbnails, setThumbnails] = useState(false)
  const [visible, setVisible] = useState(true)
  const [drawing, setDrawing] = useState(false)
  const [clearAll, setClearAll] = useState(false)
  const [size, setSize] = useState({ width: 0, height: 0 })
  const stage = useRef<HTMLDivElement>(null),
    dock = useRef<HTMLDivElement>(null)
  const hideTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const slide = state.slides[state.index]
  const pageHistory = slide ? (ink[slide.path] ?? EMPTY_HISTORY) : EMPTY_HISTORY
  const wake = useCallback(() => {
    setVisible(true)
    clearTimeout(hideTimer.current)
    hideTimer.current = setTimeout(() => {
      if (!dock.current?.contains(document.activeElement)) setVisible(false)
    }, 2500)
  }, [])
  useEffect(() => {
    hideTimer.current = setTimeout(() => {
      if (!dock.current?.contains(document.activeElement)) setVisible(false)
    }, 2500)
    return () => clearTimeout(hideTimer.current)
  }, [drawing, thumbnails])
  useEffect(() => {
    const node = stage.current
    if (!node) return
    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect
      if (rect) setSize({ width: rect.width, height: rect.height })
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  const changeHistory = useCallback(
    (change: (history: InkHistory) => InkHistory) => {
      if (slide)
        setInk((previous) => ({
          ...previous,
          [slide.path]: change(previous[slide.path] ?? EMPTY_HISTORY),
        }))
    },
    [slide],
  )
  const commit = (strokes: readonly InkStroke[]) =>
    changeHistory((history) => commitInk(history, strokes))
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (clearAll) return
      wake()
      if (event.key === 'Escape') {
        event.preventDefault()
        event.stopPropagation()
        void presentations.stop()
        return
      }
      if (drawing) return
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
        event.preventDefault()
        changeHistory(event.shiftKey ? redoInk : undoInk)
        return
      }
      if (event.metaKey || event.ctrlKey || event.altKey) return
      if (event.key === ' ' && event.target instanceof HTMLButtonElement) return
      switch (event.key.toLowerCase()) {
        case 'arrowright':
        case 'pagedown':
        case ' ':
          event.preventDefault()
          presentations.go(state.index + (event.shiftKey ? -1 : 1))
          break
        case 'arrowleft':
        case 'pageup':
          event.preventDefault()
          presentations.go(state.index - 1)
          break
        case 'home':
          event.preventDefault()
          presentations.go(0)
          break
        case 'end':
          event.preventDefault()
          presentations.go(state.slides.length - 1)
          break
        case 'p':
          setTool('pointer')
          break
        case 'b':
          setTool('pen')
          break
        case 'h':
          setTool('highlighter')
          break
        case 'e':
          setTool('eraser')
          break
      }
    }
    window.addEventListener('keydown', key, true)
    return () => window.removeEventListener('keydown', key, true)
  }, [state.index, state.slides.length, changeHistory, wake, drawing, clearAll])
  if (!slide) return null
  const scale = Math.min(size.width / slide.width, size.height / slide.height)
  const displayWidth = slide.width * scale,
    displayHeight = slide.height * scale
  return createPortal(
    <div
      className="fixed inset-0 z-50 flex bg-black text-white"
      role="dialog"
      aria-modal="true"
      aria-label={`${state.title}，演示模式`}
      onPointerMove={wake}
      onPointerDown={wake}
    >
      {thumbnails && (
        <nav
          className="z-10 w-52 shrink-0 overflow-y-auto border-r border-white/10 bg-neutral-950 p-3 pb-28"
          aria-label="跳转演示页"
        >
          {state.slides.map((item, index) => (
            <button
              key={item.path}
              className={`mb-3 block w-full rounded-lg p-2 text-left ${index === state.index ? 'bg-white/15' : 'hover:bg-white/10'}`}
              aria-label={`跳转第 ${index + 1} 页`}
              aria-current={index === state.index ? 'page' : undefined}
              onClick={() => presentations.go(index)}
            >
              <img
                src={item.url}
                alt=""
                className="aspect-video w-full rounded bg-white object-contain"
                draggable={false}
              />
              <span className="mt-2 block truncate text-xs">
                {index + 1}.{' '}
                {item.path
                  .split('/')
                  .at(-1)
                  ?.replace(/\.excalidraw$/, '')}
              </span>
            </button>
          ))}
        </nav>
      )}
      <div
        ref={stage}
        className="relative flex min-w-0 flex-1 items-center justify-center overflow-hidden"
      >
        <div className="relative shrink-0" style={{ width: displayWidth, height: displayHeight }}>
          <img
            src={slide.url}
            alt={`第 ${state.index + 1} 页：${slide.path}`}
            className="block h-full w-full select-none"
            draggable={false}
          />
          <AnnotationLayer
            key={slide.path}
            history={pageHistory}
            tool={tool}
            color={color}
            onCommit={commit}
            onDrawingChange={setDrawing}
          />
        </div>
      </div>
      <div
        className="pointer-events-none absolute left-4 top-4 max-w-[70vw] truncate rounded bg-black/50 px-3 py-1.5 text-xs text-white/80"
        aria-live="polite"
      >
        {slide.path
          .split('/')
          .at(-1)
          ?.replace(/\.excalidraw$/, '')}
      </div>
      <div
        ref={dock}
        onFocus={wake}
        className={`absolute inset-x-3 bottom-4 z-20 mx-auto w-fit max-w-[calc(100%-24px)] transition-opacity duration-150 motion-reduce:transition-none ${visible || drawing || thumbnails ? 'opacity-100' : 'pointer-events-none opacity-0 focus-within:opacity-100'}`}
      >
        <PlayerToolbar
          tool={tool}
          setTool={setTool}
          color={color}
          setColor={setColor}
          history={pageHistory}
          index={state.index}
          count={state.slides.length}
          go={(index) => presentations.go(index)}
          undo={() => changeHistory(undoInk)}
          redo={() => changeHistory(redoInk)}
          clear={() => commit([])}
          clearAll={() => setClearAll(true)}
          exit={() => void presentations.stop()}
          thumbnails={thumbnails}
          toggleThumbnails={() => setThumbnails(!thumbnails)}
        />
        <p className="pointer-events-none mt-2 text-center text-[11px] text-white/70">
          临时批注 · 翻页保留 · 退出后清空
        </p>
      </div>
      <ConfirmDialog
        zIndex={80}
        open={clearAll}
        onOpenChange={setClearAll}
        title="清空所有页面的临时批注？"
        description="原始课件保持不变。此操作会清空本次演示的全部批注。"
        destructive
        confirmLabel="清空全部批注"
        onConfirm={() => setInk({})}
      />
    </div>,
    document.body,
  )
}
