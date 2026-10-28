import { appFetch } from '../http'
import { readModelStream, AgentProtocolError } from './api-stream'
import type { AIConfig } from '../ai-config'
import type { ChatMessage, ToolCall, ToolExecutor } from '../ai'
import type { LocalChatActivity } from '../local-chat'

export type ApiExecution = {
  readonly messages: readonly ChatMessage[]
  readonly config: AIConfig
  readonly tools: readonly object[]
  readonly executor?: ToolExecutor
  readonly signal?: AbortSignal
  readonly maxTurns?: number
  readonly repairLimit?: number
  readonly onChunk: (text: string) => void
  readonly onThinking?: (text: string) => void
  readonly onActivity?: (activity: LocalChatActivity) => void
  readonly onUsage?: (usage: { readonly inputTokens?: number; readonly outputTokens?: number }) => void
}

async function execute(call: ToolCall, executor: ToolExecutor) {
  const args: unknown = JSON.parse(call.function.arguments || '{}')
  if (executor.executeTool) return executor.executeTool(call.function.name, args, call.id)
  const { canvasCommandSchema } = await import('../canvas-commands')
  const command = canvasCommandSchema.parse({ tool: call.function.name === 'get_canvas_elements' ? 'get_canvas' : call.function.name, arguments: args })
  switch (command.tool) {
    case 'get_canvas': return { elements: executor.getCanvasElements() }
    case 'delete_elements': return executor.deleteElements(command.arguments.ids)
    case 'update_elements': return executor.updateElements(command.arguments.elements)
    case 'add_elements':
    case 'fit_canvas':
    case 'undo':
    case 'redo': throw new AgentProtocolError('此运行未提供该画布工具。')
  }
}

export async function executeApi(options: ApiExecution): Promise<void> {
  const messages = [...options.messages]
  const attempts = new Map<string, number>()
  const limit = options.maxTurns ?? 8
  let repairs = 0
  options.onActivity?.({ type: 'status', message: '模型会话已启动' })
  for (let turn = 0; turn < limit; turn++) {
    options.signal?.throwIfAborted()
    const response = await appFetch(`${options.config.baseURL.trim().replace(/\/+$/, '')}/chat/completions`, {
      method: 'POST', signal: options.signal,
      headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${options.config.apiKey}` },
      body: JSON.stringify({ model: options.config.model, messages, stream: true,
        ...(options.executor ? { tools: options.tools, tool_choice: 'auto' } : {}),
      }),
    })
    if (!response.ok) throw new AgentProtocolError(`模型请求失败（HTTP ${response.status}），请检查服务商、网络及额度。`)
    const result = await readModelStream(response, { onText: options.onChunk, onThinking: options.onThinking, onUsage: options.onUsage })
    if (result.toolCalls.length === 0) {
      if (!result.content.trim()) throw new AgentProtocolError('模型没有返回回答或工具调用。')
      options.onActivity?.({ type: 'status', message: '模型执行完成' }); return
    }
    if (!options.executor) throw new AgentProtocolError('模型请求调用工具，但当前运行没有工具执行器。')
    messages.push({ role: 'assistant', content: result.content, tool_calls: result.toolCalls })
    for (const call of result.toolCalls) {
      options.signal?.throwIfAborted()
      const fingerprint = `${call.function.name}:${call.function.arguments}:${JSON.stringify(options.executor.getCanvasElements())}`
      const count = (attempts.get(fingerprint) ?? 0) + 1
      attempts.set(fingerprint, count)
      if (count > 2 && !call.function.name.startsWith('get_') && !call.function.name.startsWith('read_')) {
        throw new AgentProtocolError('相同操作连续重复且没有新结果，已停止本轮以避免重复修改。')
      }
      const activity = { type: 'tool' as const, id: call.id, name: call.function.name, input: call.function.arguments }
      options.onActivity?.({ ...activity, status: 'running' })
      let output: string
      try {
        const value = await execute(call, options.executor)
        output = JSON.stringify(value) ?? 'null'
        options.onActivity?.({ ...activity, status: 'completed', output: output.slice(0, 32768) })
      } catch (error) {
        if (options.signal?.aborted) throw error
        repairs += 1
        if (repairs > (options.repairLimit ?? 2)) throw new AgentProtocolError('工具修复次数已达上限，已保留已完成操作。')
        output = JSON.stringify({ error: error instanceof Error ? error.message : '工具执行失败', repairable: true })
        options.onActivity?.({ ...activity, status: 'failed', output })
      }
      messages.push({ role: 'tool', content: output.slice(0, 32768), tool_call_id: call.id })
    }
  }
  throw new AgentProtocolError(`本轮达到 ${limit} 次模型调用上限，已保留已完成的操作。`)
}
