import { describe, expect, it } from 'vitest'
import { commitInk, EMPTY_HISTORY, eraseAt, redoInk, undoInk } from './annotations'
import type { InkStroke } from './annotations'

const line: InkStroke = {
  id: 'line',
  tool: 'pen',
  color: '#e03131',
  points: [
    { x: 0.1, y: 0.5 },
    { x: 0.9, y: 0.5 },
  ],
}
const dot: InkStroke = {
  id: 'dot',
  tool: 'highlighter',
  color: '#f2b01e',
  points: [{ x: 0.5, y: 0.8 }],
}

describe('ink history', () => {
  it('restores the whole gesture when undoing', () => {
    const before = commitInk(EMPTY_HISTORY, [line])
    const history = commitInk(before, [line, dot])
    const result = undoInk(history)
    expect(result.present).toEqual([line])
    expect(result.future).toEqual([[line, dot]])
    expect(history.present).toEqual([line, dot])
  })
  it('restores strokes when redoing', () => {
    const history = undoInk(commitInk(EMPTY_HISTORY, [line, dot]))
    const result = redoInk(history)
    expect(result.present).toEqual([line, dot])
    expect(result.future).toEqual([])
  })
  it('discards the old branch when committing after undo', () => {
    const history = undoInk(commitInk(commitInk(EMPTY_HISTORY, [line]), [line, dot]))
    const result = commitInk(history, [dot])
    expect(result.past).toEqual([[], [line]])
    expect(result.present).toEqual([dot])
    expect(redoInk(result)).toBe(result)
  })
  it('preserves redo when committing unchanged strokes', () => {
    const history = undoInk(commitInk(commitInk(EMPTY_HISTORY, [line]), [line, dot]))
    const result = commitInk(history, [...history.present])
    expect(result).toBe(history)
  })
  it('preserves identity when history is empty', () => {
    expect(undoInk(EMPTY_HISTORY)).toBe(EMPTY_HISTORY)
    expect(redoInk(EMPTY_HISTORY)).toBe(EMPTY_HISTORY)
  })
  it('restores all erased strokes with one undo when a gesture removes several', () => {
    const history = commitInk(EMPTY_HISTORY, [line, dot])
    const first = eraseAt(history.present, { x: 0.5, y: 0.5 }, { width: 1000, height: 500 })
    const second = eraseAt(first, { x: 0.5, y: 0.8 }, { width: 1000, height: 500 })
    const result = undoInk(commitInk(history, second))
    expect(result.present).toEqual([line, dot])
    expect(result.future).toEqual([[]])
  })
})

describe('screen-space erasing', () => {
  it('erases a crossing between distant vertices', () => {
    const strokes = [line, dot]
    const result = eraseAt(strokes, { x: 0.5, y: 0.5 }, { width: 1000, height: 500 })
    expect(result).toEqual([dot])
    expect(strokes).toEqual([line, dot])
  })
  it.each([
    { width: 1000, height: 500 },
    { width: 400, height: 800 },
  ])('keeps the 12px hit radius after resize to $width x $height', (size) => {
    const strokes = [line]
    const inside = eraseAt(strokes, { x: 0.5, y: 0.5 + 11 / size.height }, size)
    const outside = eraseAt(strokes, { x: 0.5, y: 0.5 + 13 / size.height }, size)
    expect(inside).toEqual([])
    expect(outside).toBe(strokes)
  })
  it('uses screen-space distance on diagonal segments in a non-square slide', () => {
    const diagonal = {
      ...line,
      points: [
        { x: 0, y: 0 },
        { x: 1, y: 1 },
      ],
    }
    const result = eraseAt([diagonal], { x: 0.5, y: 0.6 }, { width: 1000, height: 100 })
    expect(result).toEqual([])
  })
  it('does not extend a segment beyond its endpoints', () => {
    const strokes = [line]
    const result = eraseAt(strokes, { x: 0, y: 0.5 }, { width: 1000, height: 500 })
    expect(result).toBe(strokes)
  })
  it('erases a click dot and repeated zero-length segments', () => {
    const repeated = { ...dot, id: 'repeated', points: [...dot.points, ...dot.points] }
    const result = eraseAt([dot, repeated], { x: 0.5, y: 0.8 }, { width: 400, height: 200 })
    expect(result).toEqual([])
  })
})
