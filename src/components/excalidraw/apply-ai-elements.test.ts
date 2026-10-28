import { expect, it, vi } from 'vitest'
import { applyAIElements } from './apply-ai-elements'

it('uses the canvas deletion operation for deletion records and applies other elements', () => {
  // Given
  const canvas = { addElements: vi.fn(), deleteElements: vi.fn(() => ({ deleted: ['shape', 'bound-text'], notFound: [] })) }
  const update = { id: 'other', strokeColor: '#f00' }
  // When
  const ids = applyAIElements(canvas, [{ id: 'shape', isDeleted: true }, update])
  // Then
  expect(canvas.deleteElements).toHaveBeenCalledWith(['shape'])
  expect(canvas.addElements).toHaveBeenCalledWith([update])
  expect(ids).toEqual(['other'])
})

it('produces the same scene when creation and deletion arrive together or in separate chunks', () => {
  // Given
  const createCanvas = () => {
    const ids = new Set<string>()
    return {
      ids,
      addElements: (elements: { id: string }[]) => { elements.forEach((element) => ids.add(element.id)) },
      deleteElements: (deleted: string[]) => { deleted.forEach((id) => ids.delete(id)); return { deleted, notFound: [] } },
    }
  }
  const combined = createCanvas(), streamed = createCanvas()
  const created = { id: 'shape', type: 'rectangle', x: 1, y: 2, width: 100, height: 80 }
  const deleted = { id: 'shape', isDeleted: true }
  // When
  applyAIElements(combined, [created, deleted])
  applyAIElements(streamed, [created])
  applyAIElements(streamed, [deleted])
  // Then
  expect([...combined.ids]).toEqual([])
  expect([...combined.ids]).toEqual([...streamed.ids])
})
