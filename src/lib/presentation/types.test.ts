import { expect, it } from 'vitest'
import { orderedPages, movePage, type Deck } from './types'
const deck: Deck = {
  projectId: 'p',
  folder: '课程',
  entries: [
    { kind: 'canvas', name: 'E10', path: '课程/E10.excalidraw' },
    { kind: 'canvas', name: 'E2', path: '课程/E2.excalidraw' },
    { kind: 'canvas', name: 'E1', path: '课程/E1.excalidraw' },
    { kind: 'canvas', name: 'extra', path: '课程/附录/图.excalidraw' },
    { kind: 'canvas', name: 'outside', path: '课程副本/其他.excalidraw' },
    { kind: 'directory', name: '附录', path: '课程/附录' },
  ],
  data: { decks: {}, areas: {} },
  settings: { order: [], includeSubfolders: false },
  selected: null,
}
it('naturally orders only direct canvas children of the requested folder', () => {
  expect(orderedPages(deck)).toEqual([
    '课程/E1.excalidraw',
    '课程/E2.excalidraw',
    '课程/E10.excalidraw',
  ])
})
it('retains custom order and appends new files while ignoring removed and duplicate entries', () => {
  expect(
    orderedPages({
      ...deck,
      settings: {
        order: ['课程/E10.excalidraw', 'missing.excalidraw', '课程/E10.excalidraw'],
        includeSubfolders: false,
      },
    }),
  ).toEqual(['课程/E10.excalidraw', '课程/E1.excalidraw', '课程/E2.excalidraw'])
})
it('includes nested pages only when requested without swallowing sibling folders', () => {
  expect(
    new Set(orderedPages({ ...deck, settings: { order: [], includeSubfolders: true } })),
  ).toEqual(
    new Set([
      '课程/E1.excalidraw',
      '课程/E2.excalidraw',
      '课程/E10.excalidraw',
      '课程/附录/图.excalidraw',
    ]),
  )
})
it('moves a page before another without losing or duplicating pages', () => {
  const original = ['a', 'b', 'c']
  expect(movePage(original, 'c', 'a')).toEqual(['c', 'a', 'b'])
  expect(original).toEqual(['a', 'b', 'c'])
})

it('moves a page downward to its neighbor and to the final position', () => {
  expect(movePage(['a', 'b', 'c'], 'a', 'b')).toEqual(['b', 'a', 'c'])
  expect(movePage(['a', 'b', 'c'], 'a', 'c')).toEqual(['b', 'c', 'a'])
})
