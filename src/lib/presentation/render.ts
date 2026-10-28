import { exportToSvg, getCommonBounds, loadFromBlob } from '@excalidraw/excalidraw'
import type {
  ExcalidrawElement,
  NonDeleted,
  OrderedExcalidrawElement,
} from '@excalidraw/excalidraw/element/types'
import { fileSchema, WorkspaceError } from '@/lib/workspace/types'
import { native } from '@/lib/workspace/documents'
import type { PlaybackArea, Slide } from './types'

export function selectionArea(
  elements: readonly ExcalidrawElement[],
  ids: readonly string[],
): PlaybackArea {
  const selected = elements.filter((element) => !element.isDeleted && ids.includes(element.id))
  if (!selected.length) throw new WorkspaceError('请先在画布上选中课件主体，再设置播放区域')
  const [x, y, right, bottom] = getCommonBounds(selected)
  return {
    x: x - 16,
    y: y - 16,
    width: Math.max(1, right - x) + 32,
    height: Math.max(1, bottom - y) + 32,
  }
}
export async function renderSlide(
  projectId: string,
  path: string,
  area?: PlaybackArea,
): Promise<Slide> {
  const file = fileSchema.parse(await native('read_canvas_file', { projectId, path }))
  const scene = await loadFromBlob(new Blob([file.content]), null, null)
  const elements = (scene.elements ?? []).filter(
    (element): element is NonDeleted<OrderedExcalidrawElement> => !element.isDeleted,
  )
  if (!elements.length) {
    const width = area?.width ?? 1600,
      height = area?.height ?? 900
    const background = document.createElementNS('http://www.w3.org/2000/svg', 'svg')
    background.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
    background.setAttribute('viewBox', `0 0 ${width} ${height}`)
    background.setAttribute('width', String(width))
    background.setAttribute('height', String(height))
    background.style.background = scene.appState?.viewBackgroundColor ?? '#ffffff'
    return {
      path,
      width,
      height,
      url: URL.createObjectURL(new Blob([background.outerHTML], { type: 'image/svg+xml' })),
    }
  }
  const [left, top] = getCommonBounds(elements)
  const padding = area ? 0 : 24
  const svg = await exportToSvg({
    elements,
    appState: {
      ...scene.appState,
      exportBackground: true,
      exportWithDarkMode: scene.appState?.theme === 'dark',
    },
    files: scene.files ?? {},
    exportPadding: padding,
  })
  if (area) {
    svg.setAttribute('viewBox', `${area.x - left} ${area.y - top} ${area.width} ${area.height}`)
    svg.setAttribute('width', String(area.width))
    svg.setAttribute('height', String(area.height))
    // The exported background rect follows content bounds; the page background must cover a custom crop too.
    svg.style.background = scene.appState?.viewBackgroundColor ?? '#ffffff'
  }
  return {
    path,
    width: Number(svg.getAttribute('width')),
    height: Number(svg.getAttribute('height')),
    url: URL.createObjectURL(new Blob([svg.outerHTML], { type: 'image/svg+xml' })),
  }
}
