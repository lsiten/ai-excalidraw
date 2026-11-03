import { beforeEach, expect, it, vi } from 'vitest'
import { workspace } from '@/lib/workspace/store'
import { PresentationController } from './controller'
import JSZip from 'jszip'
import type { Deck } from './types'
const boundary = vi.hoisted(() => ({
  invoke: vi.fn(),
  fullscreen: vi.fn(),
  render: vi.fn(),
  renderImage: vi.fn(),
  revoke: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: boundary.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isFullscreen: async () => false, setFullscreen: boundary.fullscreen }),
}))
vi.mock('@excalidraw/excalidraw', () => ({ loadFromBlob: vi.fn() }))
vi.mock('./render', () => ({ renderSlide: boundary.render, renderSlideImage: boundary.renderImage, selectionArea: vi.fn() }))
const deck: Deck = {
  projectId: 'p',
  folder: '课程',
  entries: [
    { path: '课程/E01.excalidraw', name: 'E01', kind: 'canvas' },
    { path: '课程/E02.excalidraw', name: 'E02', kind: 'canvas' },
  ],
  settings: { order: [], includeSubfolders: false },
  data: { areas: {}, decks: {} },
  selected: null,
}
let controller: PresentationController
let calls: string[]
beforeEach(() => {
  vi.clearAllMocks()
  controller = new PresentationController()
  calls = []
  workspace.set({
    initialized: true,
    busy: false,
    aiBusy: false,
    presenting: false,
    documents: [],
    activeId: null,
  })
  boundary.invoke.mockImplementation(
    async (_name: string, input: { request: { tool: string } }) => {
      calls.push(input.request.tool)
      switch (input.request.tool) {
        case 'read_presentation':
          return { decks: {}, areas: {} }
        case 'list_files':
          return { entries: deck.entries }
        default:
          return { saved: true }
      }
    },
  )
  boundary.fullscreen.mockResolvedValue(undefined)
  boundary.render.mockImplementation(async (_project: string, path: string) => ({
    path,
    url: `blob:${path}`,
    width: 1600,
    height: 900,
  }))
  boundary.renderImage.mockImplementation(async (_project: string, path: string) => ({
    path, width: 1600, height: 900,
    data: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aMsAAAAAASUVORK5CYII=',
  }))
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(boundary.revoke)
})
it('uses a separate render path and prevents project edits during playback', async () => {
  await controller.prepare('p', '课程')
  await controller.start(deck, '课程/E02.excalidraw')
  expect(controller.getSnapshot()).toMatchObject({ phase: 'playing', index: 1 })
  await expect(workspace.run(async () => 'mutation')).rejects.toThrow('退出演示')
  expect(calls.filter((tool) => tool === 'save_canvas_file')).toEqual([])
  await controller.stop()
})
it('disposes every slide and unlocks editing when playback ends', async () => {
  await controller.prepare('p', '课程')
  await controller.start(deck, null)
  await controller.stop()
  expect(controller.getSnapshot().phase).toBe('closed')
  expect(boundary.revoke.mock.calls.map((args) => args[0])).toEqual([
    'blob:课程/E01.excalidraw',
    'blob:课程/E02.excalidraw',
  ])
  expect(boundary.fullscreen.mock.calls.map((args) => args[0])).toEqual([true, false])
  expect(workspace.getSnapshot().presenting).toBe(false)
})
it('retains setup and releases partial previews when a page cannot render', async () => {
  boundary.render
    .mockResolvedValueOnce({
      path: '课程/E01.excalidraw',
      url: 'blob:one',
      width: 1600,
      height: 900,
    })
    .mockRejectedValueOnce(new Error('Missing file'))
  await controller.prepare('p', '课程')
  await controller.start(deck, null)
  expect(controller.getSnapshot()).toMatchObject({
    phase: 'setup',
    loading: null,
    error: 'Missing file',
  })
  expect(boundary.revoke).toHaveBeenCalledWith('blob:one')
  expect(workspace.getSnapshot().presenting).toBe(false)
})

it('exports only the requested folder in saved order as a real PowerPoint package', async () => {
  let exported: readonly number[] = []
  boundary.invoke.mockImplementation(async (_name: string, input: { request: { tool: string; arguments: { bytes?: number[] } } }) => {
    switch (input.request.tool) {
      case 'read_presentation': return { decks: { '课程': { order: ['课程/E02.excalidraw', '课程/E01.excalidraw'], includeSubfolders: false } }, areas: {} }
      case 'list_files': return { entries: [...deck.entries,
        { path: '课程/附录/E03.excalidraw', name: 'E03', kind: 'canvas' },
        { path: '其他/E04.excalidraw', name: 'E04', kind: 'canvas' }] }
      case 'export_presentation':
        exported = input.request.arguments.bytes ?? []
        return { path: '/tmp/课程.pptx' }
      default: return { saved: true }
    }
  })

  await controller.exportFolder('p', '课程')

  const zip = await JSZip.loadAsync(new Uint8Array(exported))
  expect(Object.keys(zip.files).filter((path) => /^ppt\/slides\/slide\d+\.xml$/.test(path))).toHaveLength(2)
  expect(await zip.file('ppt/notesSlides/notesSlide1.xml')?.async('string')).toContain('课程/E02.excalidraw')
  expect(await zip.file('ppt/notesSlides/notesSlide2.xml')?.async('string')).toContain('课程/E01.excalidraw')
  expect(controller.getSnapshot()).toMatchObject({ phase: 'setup', loading: null, error: null, notice: '已导出 2 页：/tmp/课程.pptx' })
  expect(workspace.getSnapshot().busy).toBe(false)
})

it('does not offer a partial PPT file when a canvas fails to render', async () => {
  boundary.renderImage.mockRejectedValueOnce(new Error('图片不可读取'))
  await controller.exportFolder('p', '课程')
  expect(calls).not.toContain('export_presentation')
  expect(controller.getSnapshot()).toMatchObject({ phase: 'setup', loading: null, error: '无法导出 课程/E01.excalidraw：图片不可读取' })
  expect(workspace.getSnapshot().busy).toBe(false)
})
