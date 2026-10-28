import { beforeEach, expect, it, vi } from 'vitest'
import { Workspace } from './store'
import type { CanvasFile } from './types'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@excalidraw/excalidraw', () => ({
  loadFromBlob: vi.fn().mockResolvedValue({ elements: [], appState: {}, files: {} }),
}))
const makeFile = (path: string): CanvasFile => ({
  tabId: `tab-${path}`,
  id: `p:${path}`,
  projectId: 'p',
  path,
  content: 'original',
  savedContent: 'original',
  revision: 'v1',
  chats: [],
  savedChats: '[]',
  scene: { elements: [], appState: {}, files: {} },
  status: 'saved',
})
let workspace: Workspace
let disk: Map<string, string>
let duringSave: (path: string) => void
let duringRename: () => void
interface Request {
  tool: string
  arguments: { path?: string; content?: string; from?: string; to?: string }
}
beforeEach(() => {
  workspace = new Workspace()
  disk = new Map([
    ['A.excalidraw', 'original'],
    ['B.excalidraw', 'original'],
  ])
  duringSave = () => {}
  duringRename = () => {}
  invoke.mockReset()
  invoke.mockImplementation(async (_command: string, { request }: { request: Request }) => {
    const { tool, arguments: args } = request
    if (tool === 'save_canvas_file') {
      const path = args.path ?? ''
      duringSave(path)
      disk.set(path, args.content ?? '')
      return { revision: 'v2' }
    }
    if (tool === 'rename_file') {
      duringRename()
      disk.set(args.to ?? '', disk.get(args.from ?? '') ?? '')
      disk.delete(args.from ?? '')
      return { from: args.from, to: args.to }
    }
    if (tool === 'list_files') return { entries: [] }
    return { saved: true }
  })
  workspace.set({
    initialized: true,
    projectId: 'p',
    documents: [makeFile('A.excalidraw'), { ...makeFile('B.excalidraw'), content: 'dirty-B' }],
    activeId: 'p:A.excalidraw',
  })
})
it('drains edits arriving on an earlier tab while another tab saves', async () => {
  duringSave = (path) => {
    if (path === 'B.excalidraw') workspace.documents.scene('p:A.excalidraw', 'late-A')
  }
  await workspace.run(() => workspace.flushAll())
  expect(disk.get('A.excalidraw')).toBe('late-A')
  expect(workspace.file('p:A.excalidraw').savedContent).toBe('late-A')
})
it('preserves late rename edits, editor identity and writes only to the new file', async () => {
  duringRename = () => workspace.documents.scene('p:A.excalidraw', 'late-A')
  await workspace.run(() =>
    workspace.mutate('rename_file', {
      projectId: 'p',
      from: 'A.excalidraw',
      to: 'Renamed.excalidraw',
    }),
  )
  const renamed = workspace.file('p:Renamed.excalidraw')
  expect(renamed.content).toBe('late-A')
  expect(renamed.savedContent).toBe('late-A')
  expect(renamed.tabId).toBe('tab-A.excalidraw')
  expect(disk.get('Renamed.excalidraw')).toBe('late-A')
  expect(disk.has('A.excalidraw')).toBe(false)
  expect(workspace.documents.timers.has('p:A.excalidraw')).toBe(false)
})
it('requires saving by default and permits explicitly discarding a missing conflicted tab', async () => {
  workspace.documents.access.update('p:A.excalidraw', {
    status: 'conflict',
    error: 'file is missing',
    content: 'local edit',
  })
  await expect(workspace.close('p:A.excalidraw')).rejects.toThrow('file is missing')
  await workspace.close('p:A.excalidraw', true)
  expect(workspace.getSnapshot().documents.map((doc) => doc.id)).toEqual(['p:B.excalidraw'])
})

it('allows the bound local agent through the AI lock while retaining the user-operation lock', async () => {
  // Given
  workspace.set({ aiBusy: true, localAgentCanvasId: 'p:A.excalidraw' })
  const action = vi.fn().mockResolvedValue('done')
  // When
  const result = await workspace.run(action, 'p:A.excalidraw')
  // Then
  expect(result).toBe('done')
  expect(workspace.getSnapshot().aiBusy).toBe(true)
  expect(workspace.getSnapshot().busy).toBe(false)
})

it.each([undefined, 'p:B.excalidraw'])('rejects unrelated operations during a local agent run (%s)', async (target) => {
  // Given
  workspace.set({ aiBusy: true, localAgentCanvasId: 'p:A.excalidraw' })
  const action = vi.fn()
  // When / Then
  await expect(workspace.run(action, target)).rejects.toThrow()
  expect(action).not.toHaveBeenCalled()
})

it.each([{ busy: true }, { presenting: true }])('keeps serialization and presentation protection for the bound agent (%j)', async (guard) => {
  // Given
  workspace.set({ aiBusy: true, localAgentCanvasId: 'p:A.excalidraw', ...guard })
  const action = vi.fn()
  // When / Then
  await expect(workspace.run(action, 'p:A.excalidraw')).rejects.toThrow()
  expect(action).not.toHaveBeenCalled()
})
