import type { LibraryItem } from '@excalidraw/excalidraw/types'
import type { ExcalidrawElement } from '@excalidraw/excalidraw/element/types'
/**
 * 从 URL hash 解析 addLibrary 参数
 * 格式：#addLibrary=<encodedURL>&token=<token>
 */
export function parseAddLibraryFromHash(): { libraryUrl: string; token: string } | null {
  const hash = window.location.hash.slice(1)
  if (!hash) return null

  const params = new URLSearchParams(hash)
  const libraryUrl = params.get('addLibrary')
  const token = params.get('token') || ''

  if (!libraryUrl) return null

  try {
    const url = new URL(libraryUrl)
    if (url.protocol !== 'https:') return null
  } catch {
    return null
  }

  return { libraryUrl, token }
}

export async function fetchLibraryItems(url: string): Promise<LibraryItem[]> {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`Failed to fetch library: ${res.status}`)

  const data = await res.json()
  if (data.type !== 'excalidrawlib' || !Array.isArray(data.library)) {
    throw new Error('Invalid excalidrawlib format')
  }

  return data.library.map((elements: ExcalidrawElement[], index: number) => ({
    status: 'unpublished' as const,
    id: crypto.randomUUID ? crypto.randomUUID() : `lib-${Date.now()}-${index}`,
    created: Date.now(),
    elements,
  }))
}
