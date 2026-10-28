import { Channel, invoke, isTauri } from '@tauri-apps/api/core'
import { z } from 'zod'
import type { IntegrationId } from './integrations'

const localToolSchema = z.object({
  id: z.enum(['codex', 'claude', 'cursor', 'pi']),
  name: z.string(),
  available: z.boolean(),
  note: z.string().nullish(),
  capabilities: z.record(z.unknown()).optional(),
}).readonly()
export type LocalChatTool = z.infer<typeof localToolSchema>

export const localActivitySchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('tool'), id: z.string(), name: z.string(), status: z.enum(['running', 'completed', 'failed']), input: z.string().optional(), output: z.string().optional() }),
  z.object({ type: z.literal('status'), message: z.string(), level: z.enum(['info', 'error']).optional() }),
])
export type LocalChatActivity = z.infer<typeof localActivitySchema>
export const localReplyPartSchema = z.union([
  localActivitySchema,
  z.object({ type: z.literal('chunk'), text: z.string() }),
  z.object({ type: z.literal('thinking'), text: z.string() }),
])
export type LocalReplyPart = z.infer<typeof localReplyPartSchema>
export type AIProjectContext = {
  readonly projectId: string
  readonly canvasId: string
  readonly path: string
  readonly directory: string
}

export const nativeControlSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('session'), conversationId: z.string(), sessionId: z.string(), backend: z.string(), resumed: z.boolean() }),
  z.object({ type: z.literal('phase'), phase: z.string() }),
  z.object({ type: z.literal('plan'), explanation: z.string().nullish(), steps: z.array(z.object({ step: z.string(), status: z.string() })) }),
  z.object({ type: z.literal('usage'), inputTokens: z.number(), outputTokens: z.number(), cachedInputTokens: z.number(), totalTokens: z.number() }),
  z.object({ type: z.literal('input'), requestId: z.string(), kind: z.string(), message: z.string(), supported: z.boolean() }),
])
export type NativeControlEvent = z.infer<typeof nativeControlSchema>
const eventSchema = z.union([
  localActivitySchema,
  z.object({ type: z.literal('chunk'), text: z.string() }),
  z.object({ type: z.literal('thinking'), text: z.string() }),
  z.object({ type: z.literal('error'), message: z.string() }),
  nativeControlSchema,
])

export async function getLocalChatTools(): Promise<readonly LocalChatTool[]> {
  if (!isTauri()) return []
  return z.array(localToolSchema).readonly().parse(await invoke('local_chat_tools'))
}

export class LocalChatError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'LocalChatError'
  }
}

export async function streamLocalChat(options: {
  readonly tool: IntegrationId
  readonly prompt: string
  readonly onChunk: (text: string) => void
  readonly onThinking?: (text: string) => void
  readonly onActivity?: (activity: LocalChatActivity) => void
  readonly context?: AIProjectContext
  readonly conversationId?: string
  readonly runId?: string
  readonly onControl?: (event: NativeControlEvent) => void
  readonly signal?: AbortSignal
}): Promise<void> {
  if (options.signal?.aborted) return
  if (!isTauri()) throw new LocalChatError('本机 AI 工具仅在桌面应用中可用，请使用模型 API 或打开桌面应用。')
  const requestId = options.runId ?? crypto.randomUUID()
  let streamError: Error | undefined
  const onEvent = new Channel<unknown>((payload) => {
    if (options.signal?.aborted || streamError) return
    const parsed = eventSchema.safeParse(payload)
    if (!parsed.success) {
      streamError = new LocalChatError('本机 AI 工具返回了无法识别的数据，请更新应用后重试。')
      return
    }
    switch (parsed.data.type) {
      case 'chunk': options.onChunk(parsed.data.text); break
      case 'thinking': options.onThinking?.(parsed.data.text); break
      case 'tool':
      case 'status': options.onActivity?.(parsed.data); break
      case 'error': streamError = new LocalChatError(parsed.data.message); break
      case 'session':
      case 'phase':
      case 'plan':
      case 'usage':
      case 'input': options.onControl?.(parsed.data); break
    }
  })
  const cancel = () => {
    void invoke('local_chat_cancel', { requestId }).catch((error: unknown) => {
      console.warn('Cannot stop local AI tool', error)
    })
  }
  // Dispatch start before subscribing so even an immediate stop has a registered request to cancel.
  const running = invoke('local_chat_start', {
    requestId, tool: options.tool, prompt: options.prompt, onEvent,
    projectId: options.context?.projectId,
    canvasId: options.context?.canvasId,
    conversationId: options.conversationId,
  })
  options.signal?.addEventListener('abort', cancel, { once: true })
  if (options.signal?.aborted) cancel()
  try {
    await running
    if (streamError) throw streamError
  } catch (error) {
    if (!options.signal?.aborted) {
      throw error instanceof Error ? error : new LocalChatError(String(error))
    }
  } finally {
    options.signal?.removeEventListener('abort', cancel)
    onEvent.onmessage = () => undefined
  }
}
