import { beforeEach, expect, it, vi } from 'vitest'
import { workspace } from '@/lib/workspace/store'
import { PresentationController } from './controller'
import type { Deck } from './types'
const boundary = vi.hoisted(() => ({
  invoke: vi.fn(),
  fullscreen: vi.fn(),
  render: vi.fn(),
  revoke: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({ invoke: boundary.invoke, isTauri: () => true }))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ isFullscreen: async () => false, setFullscreen: boundary.fullscreen }),
}))
vi.mock('@excalidraw/excalidraw', () => ({ loadFromBlob: vi.fn() }))
vi.mock('./render', () => ({ renderSlide: boundary.render, selectionArea: vi.fn() }))
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
