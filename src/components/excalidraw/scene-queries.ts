import type { RefObject } from 'react'
import { getCommonBounds } from '@excalidraw/excalidraw'
import type { ExcalidrawImperativeAPI } from '@excalidraw/excalidraw/types'
import type { ElementSummary } from './wrapper'
import { summarizeElement } from './scene-summary'
export function sceneQueries(excalidrawAPIRef: RefObject<ExcalidrawImperativeAPI | null>) {
  return {
    getElements: () => {
      const api = excalidrawAPIRef.current
      return api ? api.getSceneElements() : []
    },
    getCanvasState: () => {
      const api = excalidrawAPIRef.current
      if (!api) return []
      const allElements = api.getSceneElements()
      return allElements.map((el) => summarizeElement(el, allElements))
    },
    getSelectedElementsSummary: () => {
      const api = excalidrawAPIRef.current
      if (!api) return []
      try {
        const appState = api.getAppState()
        const selectedElementIds = appState?.selectedElementIds || {}
        const allElements = api.getSceneElements()
        const allElementsMap = new Map(allElements.map((el) => [el.id, el]))

        const selectedElements = allElements.filter((el) => selectedElementIds[el.id] === true)

        const resultIds = new Set<string>()
        const result: ElementSummary[] = []

        for (const el of selectedElements) {
          if (!resultIds.has(el.id)) {
            resultIds.add(el.id)
            result.push(summarizeElement(el, allElements))
          }

          if (el.boundElements && Array.isArray(el.boundElements)) {
            for (const bound of el.boundElements) {
              if (!resultIds.has(bound.id)) {
                const boundEl = allElementsMap.get(bound.id)
                if (boundEl) {
                  resultIds.add(bound.id)
                  result.push(summarizeElement(boundEl, allElements))
                }
              }
            }
          }
        }

        return result
      } catch {
        return []
      }
    },
    getCanvasBounds: () => {
      const api = excalidrawAPIRef.current
      if (!api) return null
      const elements = api.getSceneElements().filter((el) => !el.isDeleted)
      if (elements.length === 0) return null
      try {
        const [minX, minY, maxX, maxY] = getCommonBounds(elements)
        return { minX, minY, maxX, maxY }
      } catch {
        return null
      }
    },
    scrollToElements: (ids?: string[]) => {
      const api = excalidrawAPIRef.current
      if (!api) return
      const all = api.getSceneElements()
      const target = ids && ids.length > 0 ? all.filter((el) => ids.includes(el.id)) : all
      if (target.length === 0) return
      api.scrollToContent(target, { fitToContent: true, animate: true })
    },
  }
}
