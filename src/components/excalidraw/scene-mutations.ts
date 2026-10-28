import type { RefObject } from 'react'
import { CaptureUpdateAction, newElementWith } from '@excalidraw/excalidraw'
import type { ExcalidrawImperativeAPI } from '@excalidraw/excalidraw/types'
import type { ExcalidrawElement } from '@excalidraw/excalidraw/element/types'
import { getDefaultElementProps, getTypeSpecificProps, type ParsedElement } from './element-parser'
export function sceneMutations(
  excalidrawAPIRef: RefObject<ExcalidrawImperativeAPI | null>,
  clearStorage: () => void,
) {
  return {
    addElements: (newElements: ParsedElement[]) => {
      const api = excalidrawAPIRef.current
      if (!api || !newElements || newElements.length === 0) return

      const currentElements = api.getSceneElements()
      const existingElementsMap = new Map(currentElements.map((el) => [el.id, el]))

      const elementsToUpdate: ParsedElement[] = []
      const elementsToAdd: ParsedElement[] = []
      const validTypes = ['rectangle', 'ellipse', 'diamond', 'text', 'arrow', 'line']

      newElements.forEach((el) => {
        if (existingElementsMap.has(el.id)) {
          elementsToUpdate.push(el)
        } else if (
          el.type &&
          validTypes.includes(el.type as string) &&
          typeof el.x === 'number' &&
          typeof el.y === 'number'
        ) {
          elementsToAdd.push(el)
        }
      })

      let updatedElements = [...currentElements] as ExcalidrawElement[]

      if (elementsToUpdate.length > 0) {
        updatedElements = updatedElements.map((existingEl) => {
          const update = elementsToUpdate.find((el) => el.id === existingEl.id)
          if (!update) return existingEl
          return newElementWith(existingEl, { ...update, type: existingEl.type })
        })
      }

      if (elementsToAdd.length > 0) {
        const newElementsWithDefaults = elementsToAdd.map(
          (el) =>
            ({
              ...getDefaultElementProps(),
              ...getTypeSpecificProps(el.type as string, el),
              ...el,
            }) as ExcalidrawElement,
        )
        updatedElements = [...updatedElements, ...newElementsWithDefaults]
      }

      api.updateScene({ elements: updatedElements, captureUpdate: CaptureUpdateAction.IMMEDIATELY })
    },
    clearCanvas: () => {
      const api = excalidrawAPIRef.current
      if (!api) return
      api.updateScene({ elements: [], captureUpdate: CaptureUpdateAction.IMMEDIATELY })
      clearStorage()
    },
    deleteElements: (ids: string[]) => {
      const api = excalidrawAPIRef.current
      if (!api) return { deleted: [], notFound: ids }

      const currentElements = api.getSceneElements()
      const existingIds = new Set(currentElements.map((el) => el.id))

      const toDelete = new Set<string>()
      const notFound: string[] = []

      for (const id of ids) {
        if (existingIds.has(id)) toDelete.add(id)
        else notFound.push(id)
      }

      for (const el of currentElements) {
        if (toDelete.has(el.id) && el.boundElements && Array.isArray(el.boundElements)) {
          for (const bound of el.boundElements) {
            if (existingIds.has(bound.id)) toDelete.add(bound.id)
          }
        }
      }

      const remainingElements = currentElements.filter((el) => !toDelete.has(el.id))
      api.updateScene({
        elements: remainingElements,
        captureUpdate: CaptureUpdateAction.IMMEDIATELY,
      })

      return { deleted: Array.from(toDelete), notFound }
    },
    updateElements: (elements: ParsedElement[]) => {
      const api = excalidrawAPIRef.current
      if (!api) {
        return { updated: [], notFound: elements.map((e) => e.id) }
      }

      const currentElements = api.getSceneElements()
      const existingIds = new Set(currentElements.map((el) => el.id))
      const updates = new Map(elements.map((el) => [el.id, el]))
      const updated: string[] = []
      const notFound: string[] = []

      for (const el of elements) {
        if (existingIds.has(el.id)) updated.push(el.id)
        else notFound.push(el.id)
      }

      if (updated.length === 0) return { updated, notFound }

      const next = currentElements.map((existingEl) => {
        const update = updates.get(existingEl.id)
        if (!update) return existingEl
        return newElementWith(existingEl, { ...update, type: existingEl.type })
      })

      api.updateScene({ elements: next, captureUpdate: CaptureUpdateAction.IMMEDIATELY })
      return { updated, notFound }
    },
  }
}
