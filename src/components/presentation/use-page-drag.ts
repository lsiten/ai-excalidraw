import { useRef, useState } from 'react'
import type { MouseEvent, PointerEvent } from 'react'

export function usePageDrag({
  disabled,
  move,
  select,
}: {
  readonly disabled: boolean
  readonly move: (from: string, to: string) => void
  readonly select: (path: string) => void
}) {
  const [dragged, setDragged] = useState<string | null>(null)
  const [target, setTarget] = useState<string | null>(null)
  const gesture = useRef<{
    path: string
    id: number
    x: number
    y: number
    active: boolean
  } | null>(null)
  const suppressClick = useRef(false)
  const list = useRef<HTMLElement>(null)
  const pointTarget = (x: number, y: number) =>
    document.elementFromPoint(x, y)?.closest<HTMLElement>('[data-slide-path]')?.dataset.slidePath ??
    null
  const start = (event: PointerEvent<HTMLDivElement>, path: string) => {
    if (disabled || event.button !== 0) return
    suppressClick.current = false
    if (event.target instanceof Element && event.target.closest('[data-page-arrow]')) return
    event.preventDefault()
    gesture.current = {
      path,
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      active: false,
    }
    suppressClick.current = false
    event.currentTarget.setPointerCapture(event.pointerId)
  }
  const update = (event: PointerEvent<HTMLDivElement>) => {
    const current = gesture.current
    if (!current || current.id !== event.pointerId) return
    if (!current.active && Math.hypot(event.clientX - current.x, event.clientY - current.y) < 6)
      return
    current.active = true
    setDragged(current.path)
    setTarget(pointTarget(event.clientX, event.clientY))
    const node = list.current
    if (node) {
      const bounds = node.getBoundingClientRect()
      if (event.clientY < bounds.top + 48) node.scrollTop -= 12
      if (event.clientY > bounds.bottom - 48) node.scrollTop += 12
    }
  }
  const finish = (event: PointerEvent<HTMLDivElement>, commit: boolean) => {
    const current = gesture.current
    if (!current || current.id !== event.pointerId) return
    gesture.current = null
    if (event.currentTarget.hasPointerCapture(event.pointerId))
      event.currentTarget.releasePointerCapture(event.pointerId)
    if (current.active) {
      suppressClick.current = commit
      const to = pointTarget(event.clientX, event.clientY)
      if (commit && to && !disabled) move(current.path, to)
    }
    setDragged(null)
    setTarget(null)
  }
  const click = (event: MouseEvent<HTMLDivElement>, path: string) => {
    if (suppressClick.current) {
      event.preventDefault()
      event.stopPropagation()
      suppressClick.current = false
      return
    }
    if (event.target === event.currentTarget && !disabled) select(path)
  }
  return { list, dragged, target, start, update, finish, click }
}
