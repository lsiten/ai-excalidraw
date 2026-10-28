import { afterEach, describe, expect, it, vi } from 'vitest'
import { Channel, invoke, isTauri } from '@tauri-apps/api/core'
import { getLocalChatTools, streamLocalChat } from './local-chat'

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: vi.fn(() => true),
  Channel: class {
    onmessage: (event: unknown) => void
    constructor(onmessage: (event: unknown) => void) { this.onmessage = onmessage }
  },
}))
afterEach(() => { vi.clearAllMocks(); vi.mocked(isTauri).mockReturnValue(true) })

describe('local chat transport', () => {
  it('sends reply chunks through the provided callback', async () => {
    // Given
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === 'local_chat_start' && args && 'onEvent' in args && args.onEvent instanceof Channel) {
        args.onEvent.onmessage({ type: 'chunk', text: 'hello' })
      }
    })
    const onChunk = vi.fn()
    // When
    await streamLocalChat({ tool: 'pi', prompt: 'input', onChunk })
    // Then
    expect(onChunk).toHaveBeenCalledWith('hello')
  })

  it('cancels the same request and ignores chunks after stopping', async () => {
    // Given
    const abort = new AbortController()
    const onChunk = vi.fn()
    let startedId: unknown
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === 'local_chat_start' && args && 'requestId' in args && 'onEvent' in args && args.onEvent instanceof Channel) {
        startedId = args.requestId
        await Promise.resolve()
        abort.abort()
        args.onEvent.onmessage({ type: 'chunk', text: 'late' })
      }
    })
    // When
    await streamLocalChat({ tool: 'codex', prompt: 'input', onChunk, signal: abort.signal })
    // Then
    expect(invoke).toHaveBeenCalledWith('local_chat_cancel', { requestId: startedId })
    expect(onChunk).not.toHaveBeenCalled()
  })

  it('never starts an already cancelled request', async () => {
    // Given
    const abort = new AbortController()
    abort.abort()
    // When
    await streamLocalChat({ tool: 'claude', prompt: 'input', onChunk: vi.fn(), signal: abort.signal })
    // Then
    expect(invoke).not.toHaveBeenCalled()
  })

  it('rejects invalid protocol data rather than rendering raw JSON', async () => {
    // Given
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === 'local_chat_start' && args && 'onEvent' in args && args.onEvent instanceof Channel) args.onEvent.onmessage({ invalid: true })
    })
    const onChunk = vi.fn()
    // When / Then
    await expect(streamLocalChat({ tool: 'cursor', prompt: 'input', onChunk })).rejects.toThrow()
    expect(onChunk).not.toHaveBeenCalled()
  })

  it('does not invoke native tool detection in the web app', async () => {
    // Given
    vi.mocked(isTauri).mockReturnValue(false)
    // When
    const tools = await getLocalChatTools()
    // Then
    expect(tools).toEqual([])
    expect(invoke).not.toHaveBeenCalled()
  })

  it('keeps thinking and tool results separate from drawable assistant text', async () => {
    // Given
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === 'local_chat_start' && args && 'onEvent' in args && args.onEvent instanceof Channel) {
        args.onEvent.onmessage({ type: 'thinking', text: '{"id":"not-a-drawing"}' })
        args.onEvent.onmessage({ type: 'tool', id: 'call-1', name: 'get_canvas', status: 'completed', output: '{"id":"also-not-a-drawing"}' })
        args.onEvent.onmessage({ type: 'chunk', text: 'done' })
      }
    })
    const onChunk = vi.fn(), onThinking = vi.fn(), onActivity = vi.fn()
    // When
    await streamLocalChat({ tool: 'codex', prompt: 'input', onChunk, onThinking, onActivity,
      context: { projectId: 'project-A', canvasId: 'project-A:slides/one.excalidraw', path: 'slides/one.excalidraw', directory: '/tmp/project-A' } })
    // Then
    expect(onChunk).toHaveBeenCalledExactlyOnceWith('done')
    expect(onThinking).toHaveBeenCalledExactlyOnceWith('{"id":"not-a-drawing"}')
    expect(onActivity).toHaveBeenCalledWith(expect.objectContaining({ id: 'call-1', status: 'completed' }))
    expect(invoke).toHaveBeenCalledWith('local_chat_start', expect.objectContaining({ projectId: 'project-A', canvasId: 'project-A:slides/one.excalidraw' }))
  })
})
