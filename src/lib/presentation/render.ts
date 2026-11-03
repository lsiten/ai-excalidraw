import { convertToExcalidrawElements, exportToCanvas, exportToSvg, getCommonBounds, loadFromBlob } from '@excalidraw/excalidraw'
import type {
  ExcalidrawElement,
  NonDeleted,
  OrderedExcalidrawElement,
} from '@excalidraw/excalidraw/element/types'
import { fileSchema, WorkspaceError } from '@/lib/workspace/types'
import { native } from '@/lib/workspace/documents'
import type { PlaybackArea, Slide } from './types'
import type { SlideImage } from './pptx'

export async function renderSlideImage(projectId: string, path: string, area?: PlaybackArea): Promise<SlideImage> {
  const file = fileSchema.parse(await native('read_canvas_file', { projectId, path }))
  const scene = await loadFromBlob(new Blob([file.content]), null, null)
  const elements = (scene.elements ?? []).filter(
    (element): element is NonDeleted<OrderedExcalidrawElement> => !element.isDeleted,
  )
  const frame = area ? convertToExcalidrawElements([
    { type: 'frame', children: [], ...area },
  ]).find((element) => element.type === 'frame') : undefined
  if (area && !frame) throw new WorkspaceError('无法创建 PPT 播放区域')
  const dimensions = (width: number, height: number) => {
    const scale = Math.min(2, 2560 / Math.max(width, height))
    return { width: Math.max(1, Math.round(width * scale)), height: Math.max(1, Math.round(height * scale)), scale }
  }
  const canvas = elements.length ? await exportToCanvas({
    elements, files: scene.files ?? {}, exportingFrame: frame, exportPadding: 24,
    getDimensions: dimensions,
    appState: { ...scene.appState, exportBackground: true,
      exportWithDarkMode: scene.appState?.theme === 'dark' },
  }) : document.createElement('canvas')
  try {
    if (!elements.length) {
      const size = dimensions(area?.width ?? 1600, area?.height ?? 900)
      canvas.width = size.width
      canvas.height = size.height
      const context = canvas.getContext('2d')
      if (!context) throw new WorkspaceError('无法创建 PPT 页面图像')
      context.fillStyle = scene.appState?.viewBackgroundColor ?? '#ffffff'
      context.fillRect(0, 0, canvas.width, canvas.height)
    }
    return { path, width: canvas.width, height: canvas.height, data: canvas.toDataURL('image/png') }
  } finally {
    canvas.width = 0
    canvas.height = 0
  }
}

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
