import type { ExcalidrawElement } from '@excalidraw/excalidraw/element/types'
import { readStorage, removeStorage } from '@/lib/storage'
const STORAGE_KEY_BASE = 'excalidraw-canvas-data'
const SHARED_STORAGE_KEY = 'excalidraw-canvas-data'

export function getStorageKey(sessionId: string | null, useIndependentCanvas: boolean): string {
  if (!useIndependentCanvas || !sessionId) return SHARED_STORAGE_KEY
  return `${STORAGE_KEY_BASE}-${sessionId}`
}

export function loadCanvasData(
  sessionId: string | null,
  useIndependentCanvas: boolean,
): { elements: ExcalidrawElement[] } | null {
  if (typeof window === 'undefined') return null

  const storageKey = getStorageKey(sessionId, useIndependentCanvas)
  const parsed = readStorage<{ elements?: ExcalidrawElement[] } | null>(storageKey, null)

  if (!parsed || !Array.isArray(parsed.elements)) {
    if (parsed) removeStorage(storageKey)
    return null
  }

  const validElements = parsed.elements.filter(
    (el) => el && el.id && el.type && typeof el.x === 'number' && typeof el.y === 'number',
  )
  return { elements: validElements }
}
