import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { streamChat } from './ai'
import { getAIConfig, isConfigValid, saveAIConfig } from './ai-config'
import { streamLocalChat } from './local-chat'
import { appFetch } from './http'

vi.mock('./local-chat', () => ({ streamLocalChat: vi.fn() }))
vi.mock('./http', () => ({ appFetch: vi.fn() }))

beforeEach(() => {
  const storage = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => storage.get(key) ?? null,
    setItem: (key: string, value: string) => storage.set(key, value),
  })
  vi.stubGlobal('window', new EventTarget())
})
afterEach(() => { vi.unstubAllGlobals(); vi.unstubAllEnvs(); vi.clearAllMocks() })

describe('AI configuration', () => {
  it('preserves existing API settings when migrating an older config', () => {
    // Given
    localStorage.setItem('ai-excalidraw-config', JSON.stringify({ apiKey: 'saved-key', baseURL: 'https://example.test/v1', model: 'saved-model' }))
    // When
    const config = getAIConfig()
    // Then
    expect(config).toEqual({ provider: 'api', apiKey: 'saved-key', baseURL: 'https://example.test/v1', model: 'saved-model' })
  })

  it('retains the chosen local source when API environment settings exist', () => {
    // Given
    vi.stubEnv('VITE_AI_API_KEY', 'environment-key')
    vi.stubEnv('VITE_AI_BASE_URL', 'https://environment.test/v1')
    saveAIConfig({ provider: 'pi', apiKey: '', baseURL: '', model: 'stored-model' })
    // When
    const config = getAIConfig()
    // Then
    expect(config.provider).toBe('pi')
    expect(config.apiKey).toBe('environment-key')
  })

  it('uses a partial environment model setting when there is no saved model', () => {
    // Given
    vi.stubEnv('VITE_AI_MODEL', 'environment-model')
    // When
    const config = getAIConfig()
    // Then
    expect(config.model).toBe('environment-model')
  })

  it('does not require an API key for a local source', () => {
    // Given
    const config = { provider: 'codex', apiKey: '', baseURL: '', model: '' } as const
    // When
    const valid = isConfigValid(config)
    // Then
    expect(valid).toBe(true)
  })

  it('rejects whitespace-only API credentials', () => {
    // Given
    const config = { provider: 'api', apiKey: ' ', baseURL: 'https://example.test', model: 'model' } as const
    // When
    const valid = isConfigValid(config)
    // Then
    expect(valid).toBe(false)
  })
})

describe('chat source routing', () => {
  it('carries the exact request and selection as separate canvas feedback data', async () => {
    // Given
    const selected = [{ id: 'option-b', type: 'text', text: 'B: revised by user', x: 10, y: 20, width: 150, height: 25 }]
    vi.mocked(streamLocalChat).mockResolvedValue()
    // When
    await streamChat({ userMessage: '解释选中的方案', selectedElements: selected,
      projectContext: { projectId: 'project', canvasId: 'project:workflow.excalidraw', path: 'workflow.excalidraw', directory: '/example/project' },
      config: { provider: 'codex', apiKey: '', baseURL: '', model: '' }, onChunk: vi.fn() })
    // Then
    const options = vi.mocked(streamLocalChat).mock.calls[0]?.[0]
    if (!options) throw new Error('Local request was not sent')
    const payload = JSON.parse(options.prompt)
    const message = JSON.parse(payload.messages.at(-1).content)
    expect(payload.feedback).toEqual({ selectedElementIds: ['option-b'], selectedElements: selected })
    expect(message).toMatchObject({ request: '解释选中的方案', canvasContext: { selectedElements: selected } })
    expect(payload.context.canvasId).toBe('project:workflow.excalidraw')
  })
  it('sends a local conversation through the CLI without an HTTP API request', async () => {
    // Given
    vi.mocked(streamLocalChat).mockImplementation(async ({ onChunk }) => onChunk('reply'))
    const onChunk = vi.fn()
    // When
    await streamChat({ userMessage: 'draw', config: { provider: 'claude', apiKey: '', baseURL: '', model: '' }, onChunk })
    // Then
    expect(streamLocalChat).toHaveBeenCalledWith(expect.objectContaining({ tool: 'claude' }))
    expect(appFetch).not.toHaveBeenCalled()
    expect(onChunk).toHaveBeenCalledWith('reply')
  })

  it('reports a CLI failure without falling back to the model API', async () => {
    // Given
    const error = new Error('not signed in')
    vi.mocked(streamLocalChat).mockRejectedValue(error)
    const onError = vi.fn()
    // When
    await streamChat({ userMessage: 'draw', config: { provider: 'cursor', apiKey: '', baseURL: '', model: '' }, onChunk: vi.fn(), onError })
    // Then
    expect(onError).toHaveBeenCalledWith(error)
    expect(appFetch).not.toHaveBeenCalled()
  })
})
