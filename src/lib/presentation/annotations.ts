export type AnnotationTool = 'pointer' | 'pen' | 'highlighter' | 'eraser'

export type InkStroke = {
  readonly id: string
  readonly tool: 'pen' | 'highlighter'
  readonly color: string
  readonly points: readonly { readonly x: number; readonly y: number }[]
}

export type InkHistory = {
  readonly past: readonly (readonly InkStroke[])[]
  readonly present: readonly InkStroke[]
  readonly future: readonly (readonly InkStroke[])[]
}

export const EMPTY_HISTORY: InkHistory = { past: [], present: [], future: [] } as const

export function commitInk(history: InkHistory, strokes: readonly InkStroke[]): InkHistory {
  if (
    strokes.length === history.present.length &&
    strokes.every((stroke, index) => stroke === history.present[index])
  )
    return history
  return { past: [...history.past, history.present], present: strokes, future: [] }
}

export function undoInk(history: InkHistory): InkHistory {
  const previous = history.past.at(-1)
  if (!previous) return history
  return {
    past: history.past.slice(0, -1),
    present: previous,
    future: [history.present, ...history.future],
  }
}

export function redoInk(history: InkHistory): InkHistory {
  const next = history.future[0]
  if (!next) return history
  return {
    past: [...history.past, history.present],
    present: next,
    future: history.future.slice(1),
  }
}

export function eraseAt(
  strokes: readonly InkStroke[],
  point: InkStroke['points'][number],
  size: { readonly width: number; readonly height: number },
): readonly InkStroke[] {
  const remaining = strokes.filter(
    (stroke) =>
      !stroke.points.some((end, index) => {
        const start = stroke.points[index - 1] ?? end
        const dx = (end.x - start.x) * size.width
        const dy = (end.y - start.y) * size.height
        const px = (point.x - start.x) * size.width
        const py = (point.y - start.y) * size.height
        const lengthSquared = dx * dx + dy * dy
        const t =
          lengthSquared === 0 ? 0 : Math.max(0, Math.min(1, (px * dx + py * dy) / lengthSquared))
        return (px - t * dx) ** 2 + (py - t * dy) ** 2 <= 12 ** 2
      }),
  )
  return remaining.length === strokes.length ? strokes : remaining
}
