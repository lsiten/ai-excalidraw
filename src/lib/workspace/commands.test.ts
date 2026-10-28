import { beforeEach, expect, it, vi } from 'vitest'
vi.mock('@excalidraw/excalidraw', () => ({ loadFromBlob: vi.fn() }))
import { workspaceCommand } from './commands'
const mocks = vi.hoisted(() => ({
  execute: vi.fn(),
  ready: vi.fn(),
  activate: vi.fn(),
  flush: vi.fn(),
  file: vi.fn(),
  native: vi.fn(),
  run: vi.fn(),
  state: { activeId: 'project:B.excalidraw', documents: [], projects: [], projectId: 'project', presenting: false },
}))
vi.mock('./store', () => ({
  workspace: {
    run: mocks.run,
    getSnapshot: () => mocks.state,
    file: mocks.file,
    activate: mocks.activate,
    documents: { ready: mocks.ready },
  },
}))
vi.mock('./documents', () => ({ native: mocks.native }))
vi.mock('@/lib/canvas-commands', async () => {
  const actual =
    await vi.importActual<typeof import('@/lib/canvas-commands')>('@/lib/canvas-commands')
  return { ...actual, executeCanvasCommand: mocks.execute }
})
beforeEach(() => {
  vi.clearAllMocks()
  Reflect.deleteProperty(mocks.state, 'presenting')
  mocks.run.mockImplementation((action: () => Promise<unknown>) => action())
  mocks.file.mockImplementation((id: string) => ({
    id,
    projectId: 'project',
    path: id.split(':')[1],
    status: 'saved',
  }))
  mocks.ready.mockResolvedValue({ flushCanvas: mocks.flush })
  mocks.flush.mockResolvedValue(true)
  mocks.execute.mockReturnValue({ added: ['box'] })
})
it('rejects implicit mutations before touching the active canvas', async () => {
  await expect(
    workspaceCommand({ tool: 'delete_elements', arguments: { ids: ['box'] } }),
  ).rejects.toThrow('canvasId')
  expect(mocks.execute).not.toHaveBeenCalled()
})
it('targets the specified background document and awaits its persistence', async () => {
  await workspaceCommand({
    tool: 'add_elements',
    arguments: {
      canvasId: 'project:A.excalidraw',
      elements: [{ id: 'box', type: 'rectangle', x: 0, y: 0, width: 100, height: 80 }],
    },
  })
  expect(mocks.ready).toHaveBeenCalledWith('project:A.excalidraw')
  expect(mocks.activate).not.toHaveBeenCalled()
  expect(mocks.flush).toHaveBeenCalledTimes(1)
})
it('reports the identity of the document actually read', async () => {
  mocks.execute.mockReturnValue({ elements: [] })
  expect(await workspaceCommand({ tool: 'get_canvas', arguments: {} })).toMatchObject({
    canvasId: 'project:B.excalidraw',
    path: 'B.excalidraw',
  })
})
it('refuses writes to a document with unresolved external changes', async () => {
  mocks.file.mockReturnValue({ status: 'conflict', error: 'external change' })
  await expect(
    workspaceCommand({
      tool: 'delete_elements',
      arguments: { canvasId: 'project:A.excalidraw', ids: ['box'] },
    }),
  ).rejects.toThrow('external change')
  expect(mocks.execute).not.toHaveBeenCalled()
})
it('reports playback state even when guarded operations are unavailable', async () => {
  mocks.run.mockRejectedValue(new Error('playback active'))
  expect(await workspaceCommand({ tool: 'get_workspace' })).toMatchObject({ presenting: false })
  mocks.state.presenting = true
  expect(await workspaceCommand({ tool: 'get_workspace' })).toMatchObject({ presenting: true })
  expect(mocks.run).not.toHaveBeenCalled()
})
it('reads metadata through the native presentation command without touching a canvas', async () => {
  const presentation = { decks: {}, areas: {} }
  mocks.native.mockResolvedValue(presentation)
  expect(await workspaceCommand({ tool: 'get_presentation', arguments: { projectId: 'project' } })).toEqual(presentation)
  expect(mocks.native).toHaveBeenCalledExactlyOnceWith('read_presentation', { projectId: 'project' })
  expect(mocks.run).toHaveBeenCalledTimes(1)
  expect(mocks.ready).not.toHaveBeenCalled()
  expect(mocks.execute).not.toHaveBeenCalled()
})
it('writes root and nested folder metadata only, preserving the payload and native result', async () => {
  const presentation = {
    decks: { '': { order: ['目录/a.excalidraw'], includeSubfolders: true }, '目录': { order: [], includeSubfolders: false } },
    areas: { '目录/a.excalidraw': { x: -10, y: 20, width: 100, height: 50 } },
  }
  mocks.native.mockResolvedValue({ saved: true })
  expect(await workspaceCommand({ tool: 'set_presentation', arguments: { projectId: 'project', presentation } })).toEqual({ saved: true })
  expect(mocks.native).toHaveBeenCalledExactlyOnceWith('write_presentation', { projectId: 'project', presentation })
  expect(mocks.run).toHaveBeenCalledTimes(1)
  expect(mocks.ready).not.toHaveBeenCalled()
  expect(mocks.execute).not.toHaveBeenCalled()
  expect(mocks.flush).not.toHaveBeenCalled()
})
it.each([
  { decks: {}, areas: {}, extra: true },
  { decks: { '': { order: [], includeSubfolders: 'false' } }, areas: {} },
  { decks: { '': { order: [], includeSubfolders: false, extra: true } }, areas: {} },
  { decks: {}, areas: { 'a.excalidraw': { x: 0, y: 0, width: 0, height: 1 } } },
  { decks: {}, areas: { 'a.excalidraw': { x: Infinity, y: 0, width: 1, height: 1 } } },
  { decks: {}, areas: { 'a.excalidraw': { x: 0, y: 0, width: 1, height: 1, extra: true } } },
  { decks: {} },
])('rejects malformed presentation metadata before native dispatch: %j', async (presentation) => {
  await expect(workspaceCommand({ tool: 'set_presentation', arguments: { projectId: 'project', presentation } })).rejects.toThrow()
  expect(mocks.native).not.toHaveBeenCalled()
})
it.each(['get_presentation', 'set_presentation'])('honors workspace guard rejection for %s', async (tool) => {
  mocks.run.mockRejectedValue(new Error('playback active'))
  await expect(workspaceCommand({ tool, arguments: { projectId: 'project', ...(tool === 'set_presentation' ? { presentation: { decks: {}, areas: {} } } : {}) } })).rejects.toThrow('playback active')
  expect(mocks.native).not.toHaveBeenCalled()
})
it('propagates native validation failures without retrying or editing a canvas', async () => {
  mocks.native.mockRejectedValue(new Error('unsafe presentation path'))
  await expect(workspaceCommand({ tool: 'set_presentation', arguments: { projectId: 'project', presentation: { decks: { '../': { order: [], includeSubfolders: false } }, areas: {} } } })).rejects.toThrow('unsafe presentation path')
  expect(mocks.native).toHaveBeenCalledTimes(1)
  expect(mocks.execute).not.toHaveBeenCalled()
})
