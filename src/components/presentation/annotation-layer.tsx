import { useEffect, useRef, useState } from 'react'
import type { PointerEvent } from 'react'
import { eraseAt } from '../../lib/presentation/annotations'
import type { AnnotationTool, InkHistory, InkStroke } from '../../lib/presentation/annotations'

type AnnotationLayerProps = {
  readonly history: InkHistory
  readonly tool: AnnotationTool
  readonly color: string
  readonly onCommit: (strokes: readonly InkStroke[]) => void
  readonly onDrawingChange?: (drawing: boolean) => void
}

type Gesture = {
  readonly pointerId: number
  readonly base: readonly InkStroke[]
} & (
  | { readonly tool: 'eraser'; readonly strokes: readonly InkStroke[] }
  | { readonly tool: 'pen' | 'highlighter'; readonly stroke: InkStroke }
)

const inkStyle = {
  pen: { strokeWidth: 3, opacity: 1 },
  highlighter: { strokeWidth: 18, opacity: 0.28 },
} as const

function sample(event: PointerEvent<SVGSVGElement>) {
  const rect = event.currentTarget.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) return null
  return {
    point: {
      x: Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
      y: Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height)),
    },
    size: { width: rect.width, height: rect.height },
  }
}

export function AnnotationLayer({
  history,
  tool,
  color,
  onCommit,
  onDrawingChange,
}: AnnotationLayerProps) {
  const gesture = useRef<Gesture | null>(null)
  const [draft, setDraft] = useState<readonly InkStroke[] | null>(null)
  const drawingCallback = useRef(onDrawingChange)
  useEffect(() => {
    drawingCallback.current = onDrawingChange
  }, [onDrawingChange])
  useEffect(
    () => () => {
      drawingCallback.current?.(false)
    },
    [],
  )

  function start(event: PointerEvent<SVGSVGElement>) {
    if (event.button !== 0 || gesture.current) return
    const input = sample(event)
    if (!input) return
    const base = history.present
    switch (tool) {
      case 'pointer':
        return
      case 'eraser':
        gesture.current = {
          pointerId: event.pointerId,
          base,
          tool,
          strokes: eraseAt(base, input.point, input.size),
        }
        setDraft(gesture.current.strokes)
        break
      case 'pen':
      case 'highlighter': {
        const stroke: InkStroke = { id: crypto.randomUUID(), tool, color, points: [input.point] }
        gesture.current = { pointerId: event.pointerId, base, tool, stroke }
        setDraft([...base, stroke])
        break
      }
      default: {
        const exhaustive: never = tool
        return exhaustive
      }
    }
    event.preventDefault()
    event.stopPropagation()
    event.currentTarget.setPointerCapture(event.pointerId)
    onDrawingChange?.(true)
  }

  function move(event: PointerEvent<SVGSVGElement>) {
    const current = gesture.current
    if (!current || current.pointerId !== event.pointerId) return null
    const input = sample(event)
    if (!input) return null
    event.preventDefault()
    event.stopPropagation()
    switch (current.tool) {
      case 'eraser': {
        const strokes = eraseAt(current.strokes, input.point, input.size)
        gesture.current = { ...current, strokes }
        setDraft(strokes)
        return strokes
      }
      case 'pen':
      case 'highlighter': {
        const last = current.stroke.points.at(-1)
        const stroke =
          last?.x === input.point.x && last.y === input.point.y
            ? current.stroke
            : { ...current.stroke, points: [...current.stroke.points, input.point] }
        gesture.current = { ...current, stroke }
        const strokes = [...current.base, stroke]
        setDraft(strokes)
        return strokes
      }
      default: {
        const exhaustive: never = current
        return exhaustive
      }
    }
  }

  function finish(event: PointerEvent<SVGSVGElement>, commit: boolean) {
    const current = gesture.current
    if (!current || current.pointerId !== event.pointerId) return
    const strokes = commit ? move(event) : null
    gesture.current = null
    setDraft(null)
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId)
    onDrawingChange?.(false)
    if (strokes && strokes !== current.base) onCommit(strokes)
  }

  return (
    <svg
      aria-label="Slide annotations"
      viewBox="0 0 1000 1000"
      preserveAspectRatio="none"
      style={{
        position: 'absolute',
        inset: 0,
        width: '100%',
        height: '100%',
        pointerEvents: tool === 'pointer' ? 'none' : 'auto',
        touchAction: 'none',
      }}
      onPointerDown={start}
      onPointerMove={move}
      onPointerUp={(event) => finish(event, true)}
      onPointerCancel={(event) => finish(event, false)}
      onLostPointerCapture={(event) => finish(event, false)}
    >
      {(draft ?? history.present).map((stroke) => {
        const points = stroke.points.map((point) => `${point.x * 1000},${point.y * 1000}`)
        return (
          <path
            key={stroke.id}
            d={points.length ? `M${points.join(' L')}${points.length === 1 ? ' l0,0' : ''}` : ''}
            fill="none"
            stroke={stroke.color}
            {...inkStyle[stroke.tool]}
            vectorEffect="non-scaling-stroke"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        )
      })}
    </svg>
  )
}
