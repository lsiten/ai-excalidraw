import { beforeEach, expect, it, vi } from 'vitest'
import { executeApi } from './api-backend'
import { readModelStream } from './api-stream'
import { appFetch } from '../http'
import type { ToolExecutor } from '../ai'
vi.mock('../http', () => ({ appFetch: vi.fn() }))
beforeEach(() => vi.clearAllMocks())
const config = { provider:'api',apiKey:'test',baseURL:'https://example.test/v1',model:'test' } as const
const response = (frames: readonly object[], done = true) => new Response(frames.map(f => `data: ${JSON.stringify(f)}\n\n`).join('') + (done ? 'data: [DONE]\n\n' : ''))
const answer = { choices:[{ delta:{ content:'OK' },finish_reason:'stop' }] }
it('decodes fragmented UTF-8 and usage separately from public text and summaries', async () => {
  // Given arbitrary byte boundaries inside a Chinese UTF-8 character and a valid final frame.
  const data = new TextEncoder().encode(`data: ${JSON.stringify({choices:[{delta:{content:'你好',reasoning_content:'摘要'},finish_reason:'stop'}],usage:{prompt_tokens:12,completion_tokens:3}})}\n\ndata: [DONE]\n`)
  const stream = new ReadableStream<Uint8Array>({ start(controller) { for (const byte of data) controller.enqueue(new Uint8Array([byte])); controller.close() } })
  const onText = vi.fn(), onThinking = vi.fn(), onUsage = vi.fn()
  // When reading the actual streaming decoder.
  const result = await readModelStream(new Response(stream), {onText,onThinking,onUsage})
  // Then protocol metadata never leaks into answer text.
  expect(result.content).toBe('你好'); expect(onThinking).toHaveBeenCalledWith('摘要')
  expect(onUsage).toHaveBeenCalledWith({inputTokens:12,outputTokens:3})
})
it('rejects a truncated reply even when partial text was received', async () => {
  // Given text without a completion frame.
  const stream = response([{choices:[{delta:{content:'partial'}}]}],false)
  // When the connection ends.
  // Then partial content is not declared successful.
  await expect(readModelStream(stream,{onText:vi.fn()})).rejects.toThrow('完成事件')
})
it('awaits a real asynchronous tool result before issuing the next model turn', async () => {
  // Given a model that requests one platform tool and then answers.
  const calls = { choices:[{delta:{tool_calls:[{index:0,id:'call-read',function:{name:'get_canvas_elements',arguments:'{}'}}]},finish_reason:'tool_calls'}] }
  vi.mocked(appFetch).mockResolvedValueOnce(response([calls])).mockResolvedValueOnce(response([answer]))
  let release: ((value: unknown) => void) | undefined
  const executeTool = vi.fn(() => new Promise(resolve => { release = resolve }))
  const executor: ToolExecutor = { getCanvasElements:()=>[],deleteElements:()=>({deleted:[],notFound:[]}),updateElements:()=>({updated:[],notFound:[]}),executeTool }
  const onChunk = vi.fn(), onActivity = vi.fn()
  // When the tool remains pending.
  const pending = executeApi({messages:[],config,tools:[],executor,onChunk,onActivity})
  await vi.waitFor(() => expect(executeTool).toHaveBeenCalled())
  expect(appFetch).toHaveBeenCalledTimes(1)
  if (!release) throw new Error('Tool never started')
  release({elements:[]}); await pending
  // Then its result is in the following request and events carry the same call identity.
  expect(appFetch).toHaveBeenCalledTimes(2)
  const body = JSON.parse(String(vi.mocked(appFetch).mock.calls[1]?.[1]?.body))
  expect(body.messages.at(-1)).toEqual({role:'tool',tool_call_id:'call-read',content:'{"elements":[]}'})
  expect(onActivity).toHaveBeenCalledWith(expect.objectContaining({id:'call-read',status:'completed'}))
  expect(onChunk).toHaveBeenCalledWith('OK')
})
it('halts after its explicit model-turn budget rather than silently losing tool requests', async () => {
  // Given a model that keeps reading with a one-turn budget.
  vi.mocked(appFetch).mockResolvedValue(response([{choices:[{delta:{tool_calls:[{index:0,id:'r',function:{name:'get_canvas_elements',arguments:'{}'}}]},finish_reason:'tool_calls'}]}]))
  const executor: ToolExecutor = {getCanvasElements:()=>[],deleteElements:()=>({deleted:[],notFound:[]}),updateElements:()=>({updated:[],notFound:[]})}
  // When the budget is consumed.
  // Then the unfinished loop fails explicitly.
  await expect(executeApi({messages:[],config,tools:[],executor,onChunk:vi.fn(),maxTurns:1})).rejects.toThrow('上限')
})
