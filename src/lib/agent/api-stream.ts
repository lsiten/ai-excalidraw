import { z } from 'zod'
import type { ToolCall } from '../ai'

const frameSchema = z.object({
  error: z.object({ message: z.string().optional() }).optional(),
  usage: z.object({ prompt_tokens: z.number().optional(), completion_tokens: z.number().optional() }).nullish(),
  choices: z.array(z.object({
    finish_reason: z.string().nullish(),
    delta: z.object({
      content: z.string().nullish(), reasoning_content: z.string().nullish(), thinking: z.string().nullish(),
      tool_calls: z.array(z.object({ index: z.number().int(), id: z.string().optional(),
        function: z.object({ name: z.string().optional(), arguments: z.string().optional() }).optional(),
      })).optional(),
    }).default({}),
  })).default([]),
})

export class AgentProtocolError extends Error { override readonly name = 'AgentProtocolError' }

export type ModelStreamObserver = {
  readonly onText: (text: string) => void
  readonly onThinking?: (text: string) => void
  readonly onUsage?: (usage: { readonly inputTokens?: number; readonly outputTokens?: number }) => void
}

export async function readModelStream(response: Response, observer: ModelStreamObserver) {
  if (!response.body) throw new AgentProtocolError('模型没有返回事件流。')
  const reader = response.body.getReader(), decoder = new TextDecoder()
  const calls = new Map<number, ToolCall>()
  let buffer = '', content = '', terminal = false, bytes = 0
  const frame = (data: string) => {
    if (data === '[DONE]') { terminal = true; return }
    const parsed = frameSchema.parse(JSON.parse(data))
    if (parsed.error) throw new AgentProtocolError('模型服务报告错误，请检查模型和服务商配置。')
    if (parsed.usage) observer.onUsage?.({ inputTokens: parsed.usage.prompt_tokens, outputTokens: parsed.usage.completion_tokens })
    for (const choice of parsed.choices) {
      if (choice.finish_reason === 'length') throw new AgentProtocolError('模型输出达到长度限制，本轮结果尚未完整生成。')
      if (choice.finish_reason) terminal = true
      const delta = choice.delta
      if (delta.content) { content += delta.content; observer.onText(delta.content) }
      const thinking = delta.reasoning_content ?? delta.thinking
      if (thinking) observer.onThinking?.(thinking)
      for (const part of delta.tool_calls ?? []) {
        const previous = calls.get(part.index) ?? { id: '', type: 'function', function: { name: '', arguments: '' } }
        calls.set(part.index, { id: part.id ?? previous.id, type: 'function', function: {
          name: previous.function.name + (part.function?.name ?? ''),
          arguments: previous.function.arguments + (part.function?.arguments ?? ''),
        } })
      }
    }
  }
  const consume = (line: string) => {
    if (line.startsWith('data:')) frame(line.slice(5).trim())
  }
  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      bytes += value.byteLength
      if (bytes > 4 * 1024 * 1024) throw new AgentProtocolError('模型事件流超过 4 MiB 上限。')
      buffer += decoder.decode(value, { stream: true })
      let newline = buffer.indexOf('\n')
      while (newline >= 0) {
        consume(buffer.slice(0, newline).trimEnd()); buffer = buffer.slice(newline + 1)
        newline = buffer.indexOf('\n')
      }
    }
    buffer += decoder.decode()
    if (buffer.trim()) consume(buffer.trimEnd())
    if (!terminal) throw new AgentProtocolError('模型事件流提前结束，未收到完成事件。')
    const toolCalls = [...calls.values()]
    if (toolCalls.some(call => !call.id || !call.function.name)) throw new AgentProtocolError('模型返回了不完整的工具调用。')
    return { content, toolCalls }
  } finally { reader.releaseLock() }
}
