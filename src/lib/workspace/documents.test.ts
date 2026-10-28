import { beforeEach, describe, expect, it, vi } from 'vitest'
import { Documents } from './documents'
import type { CanvasFile } from './types'
vi.mock('@excalidraw/excalidraw', () => ({ loadFromBlob: vi.fn() }))
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
let file: CanvasFile
let documents: Documents
beforeEach(() => {
  invoke.mockReset()
  file = {
    tabId: 'tab-A',
    scene: { elements: [], appState: {}, files: {} },
    id: 'project:A.excalidraw',
    projectId: 'project',
    path: 'A.excalidraw',
    content: 'original',
    savedContent: 'original',
    revision: 'v1',
    chats: [],
    savedChats: '[]',
    status: 'saved',
  }
  documents = new Documents({
    get: () => file,
    update: (_id, patch) => {
      file = { ...file, ...patch }
    },
  })
})
describe('document persistence', () => {
  it('saves the bound file even when another tab is selected', async () => {
    file = { ...file, content: 'changed' }
    invoke.mockResolvedValue({ revision: 'v2' })
    await documents.flush(file.id)
    expect(invoke).toHaveBeenCalledWith('workspace_command', {
      request: {
        tool: 'save_canvas_file',
        arguments: {
          projectId: 'project',
          path: 'A.excalidraw',
          content: 'changed',
          revision: 'v1',
        },
      },
    })
    expect(file.savedContent).toBe('changed')
    expect(file.status).toBe('saved')
  })
  it('drains edits made while a file write is pending', async () => {
    file = { ...file, content: 'first' }
    let release: (value: unknown) => void = () => {
      throw new Error('write not started')
    }
    invoke
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            release = resolve
          }),
      )
      .mockResolvedValue({ revision: 'v3' })
    const saving = documents.flush(file.id)
    await vi.waitFor(() => expect(file.status).toBe('saving'))
    file = { ...file, content: 'second' }
    release({ revision: 'v2' })
    await saving
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(invoke.mock.calls[1]?.[1].request.arguments).toMatchObject({
      content: 'second',
      revision: 'v2',
    })
    expect(file.savedContent).toBe('second')
  })
  it('serializes overlapping explicit saves', async () => {
    file = { ...file, content: 'changed' }
    let running = 0,
      maximum = 0
    invoke.mockImplementation(async () => {
      running++
      maximum = Math.max(maximum, running)
      await new Promise((resolve) => setTimeout(resolve, 10))
      running--
      return { revision: 'v2' }
    })
    await Promise.all([
      documents.flush(file.id),
      documents.flush(file.id),
      documents.flush(file.id),
    ])
    expect(maximum).toBe(1)
    expect(invoke).toHaveBeenCalledTimes(1)
  })
  it('keeps unsaved data and blocks retries after an external conflict', async () => {
    file = { ...file, content: 'local edits' }
    invoke.mockRejectedValueOnce('CONFLICT: external change')
    await expect(documents.flush(file.id)).rejects.toThrow('CONFLICT')
    expect(file.content).toBe('local edits')
    expect(file.savedContent).toBe('original')
    expect(file.status).toBe('conflict')
    await expect(documents.flush(file.id)).rejects.toThrow('CONFLICT')
    expect(invoke).toHaveBeenCalledTimes(1)
  })
  it('persists conversation changes even without scene changes', async () => {
    file = {
      ...file,
      chats: [{ id: 'conversation', title: '工作流', messages: [], createdAt: 1, updatedAt: 1 }],
    }
    invoke.mockResolvedValue({ saved: true })
    await documents.flush(file.id)
    expect(invoke.mock.calls[0]?.[1].request).toMatchObject({
      tool: 'write_chats',
      arguments: { projectId: 'project', path: 'A.excalidraw' },
    })
    expect(file.savedChats).toBe(JSON.stringify(file.chats))
  })
})
