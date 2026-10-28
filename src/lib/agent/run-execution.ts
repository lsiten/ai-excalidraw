import { isTauri } from '@tauri-apps/api/core'
import { z } from 'zod'
import { streamChat, type StreamChatOptions } from '../ai'
import { RunController } from './controller'
import { CanvasGateway, registerCanvasGateway } from './canvas-gateway'
import { executeAgentTool } from './tools'
import { agentProjectCommand, projectCapabilityAllowed } from './project'
import { workspace } from '../workspace/store'
import { parseCanvasOutput, assertCanvasOutputComplete } from '@/components/excalidraw/element-parser'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'
import { appendReplyPart, failChatReply } from '../chat-reply'
import { getLocalChatTools, type LocalChatActivity, type LocalReplyPart, type NativeControlEvent } from '../local-chat'
import { AgentRunError } from './store'
import { selectedArtifactContext, summarizeProjectResources } from './context'
import { boundedActivity } from './text'

export type AgentReply = {
  readonly content: string; readonly thinking: string; readonly activities: LocalChatActivity[]
  readonly parts: LocalReplyPart[]; readonly runId: string
}
export type DrawRun = {
  readonly request: StreamChatOptions
  readonly conversationId: string
  readonly parentRunId?: string
  readonly canvas: ExcalidrawWrapperRef
  readonly signal: AbortSignal
  readonly onReply: (reply: AgentReply) => void
  readonly onGenerated: (ids: readonly string[]) => void
}
const phaseLabels: Readonly<Record<string,string>> = {running:'Agent 正在执行',validating:'正在校验结果',final:'正在生成最终回复',final_answer:'正在生成最终回复',commentary:'Agent 正在说明执行进度',completed:'Agent 本轮执行完成',waiting_input:'等待你的回答',interrupted:'Agent 本轮已中断'}
export async function executeDrawRun(options: DrawRun) {
  const { request, canvas } = options
  const project = request.projectContext
  const selected = canvas.getSelectedElementsSummary()
  const run = await RunController.start({ conversationId: options.conversationId, backend: request.config?.provider ?? 'api', project,
    parentRunId: options.parentRunId, capabilities: ['events', 'cancel', 'canvas_receipts', 'validation'],
    snapshot: JSON.stringify({ request: request.userMessage, project, selected: selected.slice(0, 40).map(e => ({ id: e.id, type: e.type, text: e.text?.slice(0, 120) })) }),
  })
  run.bind(options.signal)
  const signal = run.signal
  let timeout = setTimeout(() => run.cancel('budget'), 15 * 60 * 1000)
  const gateway = new CanvasGateway(run, { editor: canvas,
    revision: () => project ? workspace.file(project.canvasId).revision : String(canvas.getCanvasState().length),
    flush: async () => { if(!await canvas.flushCanvas())throw new AgentRunError('画布未就绪或保存失败。'); if (project) await workspace.documents.flush(project.canvasId) },
  })
  if (project) { registerCanvasGateway(project.canvasId, gateway); workspace.set({ localAgentCanvasId: project.canvasId }) }
  let content = '', thinking = '', parts: LocalReplyPart[] = [], processed = 0, error: Error | undefined, phase: string | undefined
  let mutationQueue: Promise<void> = Promise.resolve()
  const generated = new Set<string>()
  const reply = () => options.onReply({ content, thinking, parts, runId: run.id,
    activities: parts.filter((p): p is LocalChatActivity => p.type === 'tool' || p.type === 'status'),
  })
  const failed = (value: unknown) => { error = value instanceof Error ? value : new AgentRunError('运行失败。') }
  const record = (promise: Promise<void>) => { void promise.catch(failed) }
  const activity = (value: LocalChatActivity) => { const safe=boundedActivity(value);parts = appendReplyPart(parts, safe); reply(); record(run.activity(safe)) }
  const control = (event: NativeControlEvent) => {
    switch (event.type) {
      case 'session': record(run.append({ type: 'session', sessionId: event.sessionId, backend: request.config?.provider ?? 'codex', resumed: event.resumed })); break
      case 'phase': phase=event.phase; activity({ type: 'status', message: phaseLabels[event.phase] ?? event.phase }); break
      case 'plan': record(run.append({ type: 'plan', steps: event.steps.map(s => `[${s.status}] ${s.step}`) })); break
      case 'usage': record(run.append({ type: 'usage', inputTokens: event.inputTokens, outputTokens: event.outputTokens })); break
      case 'input': activity({ type: 'status', message: event.supported ? event.message : `${event.message}（请停止后补充消息继续）` }); break
    }
  }
  reply()
  try {
    let extraContext = JSON.stringify({ selectedArtifacts: selectedArtifactContext(project?.canvasId, selected.map(e => e.id)) }), maxTurns = 8, repairLimit = 2
    if (project && isTauri()) {
      const manifest = await agentProjectCommand({ projectId: project.projectId, op: 'manifest' })
      extraContext = JSON.stringify({ projectResources: manifest.kind==='manifest'?summarizeProjectResources(manifest,request.userMessage):manifest, selectedArtifacts: selectedArtifactContext(project.canvasId, selected.map(e => e.id)) })
      if (manifest.kind === 'manifest') {
        if (request.config?.provider && request.config.provider!=='api' && !projectCapabilityAllowed(manifest.policy,'mcp',true)) {
          throw new AgentRunError('项目策略禁用了 MCP；当前本机后端无法保证关闭其全部原生 MCP 连接，已停止启动。可选择模型 API 处理允许的画布操作。')
        }
        maxTurns = Math.min(manifest.policy.budgets.maxTurns ?? 8, 8)
        repairLimit = Math.min(manifest.policy.budgets.repairLimit ?? 2, 2)
        clearTimeout(timeout)
        timeout = setTimeout(() => run.cancel('budget'), Math.min(manifest.policy.budgets.timeoutMs ?? 15 * 60 * 1000,15 * 60 * 1000))
        await run.append({ type: 'status', level: 'info', message: `已读取项目能力清单（${manifest.skills.length} 个 Skill）` })
      }
    }
    const capabilities=[...run.current.header.capabilities]
    if (isTauri() && request.config?.provider && request.config.provider!=='api') {
      const tools=await getLocalChatTools(), backend=tools.find(tool=>tool.id===request.config?.provider)
      for (const [name,value] of Object.entries(backend?.capabilities??{})) if(typeof value==='boolean')capabilities.push(`${name}:${value}`)
    }
    const initial=z.record(z.unknown()).parse(JSON.parse(run.current.header.contextSnapshot))
    await run.append({type:'context',snapshot:JSON.stringify({...initial,resources:JSON.parse(extraContext)}),capabilities})
    await run.state('running')
    await streamChat({ ...request, conversationId: options.conversationId, runId: run.id, extraContext, maxTurns, repairLimit,
      signal, selectedElements: selected, onControl: control,
      toolExecutor: { getCanvasElements: () => canvas.getCanvasState(),
        deleteElements: ids => canvas.deleteElements(ids), updateElements: elements => canvas.updateElements(elements),
        executeTool: async (name, args, callId) => {
          await mutationQueue; signal.throwIfAborted(); if (error) throw error
          return executeAgentTool({ run, canvas: gateway, signal, request: request.userMessage }, name, args, callId)
        },
      },
      onActivity: activity,
      onUsage: usage => record(run.append({ type: 'usage', ...usage })),
      onThinking: text => { if(signal.aborted)return;thinking += text; parts = appendReplyPart(parts, { type: 'thinking', text }); reply(); run.stream({ type: 'thinking', text }) },
      onChunk: text => {
        if (signal.aborted) return
        content += text; parts = appendReplyPart(parts, { type: 'chunk', text }); reply()
        run.stream({ type: 'message', text, phase })
        const parsed = parseCanvasOutput(content, processed)
        if (parsed.elements.length) {
          processed = content.length - parsed.remainingBuffer.length
          mutationQueue = mutationQueue.then(async () => {
            signal.throwIfAborted()
            if (error) throw error
            for (const id of await gateway.apply(parsed.elements)) generated.add(id)
          }).catch(failed)
        }
      }, onError: failed,
    })
    await mutationQueue; await gateway.drain(); signal.throwIfAborted()
    if (error) throw error
    assertCanvasOutputComplete(content)
    if (Object.values(run.current.operations).some(op => op.status === 'unknown' || op.status === 'prepared')) throw new AgentRunError('有操作尚待核对，不能声明完成。')
    await run.state('validating')
    const settled = run.current.events.flatMap(({event}) => event.type==='operation' && event.status!=='prepared' ? [event] : [])
    const unrepaired = settled.some((event,index) => event.status==='failed' && !settled.slice(index+1).some(later => later.status==='applied' &&
      run.current.operations[later.operationId]?.tool===run.current.operations[event.operationId]?.tool && event.targetIds.every(id=>later.targetIds.includes(id))))
    if (unrepaired) throw new AgentRunError('有失败的修改尚未修复，不能声明任务完成。')
    if (parts.some(p => p.type === 'tool' && p.status === 'running')) throw new AgentRunError('工具未返回完成状态。')
    if(!await canvas.flushCanvas())throw new AgentRunError('画布保存未完成。')
    if (project) await workspace.documents.flush(project.canvasId)
    await run.append({ type: 'validation', ok: true, message: '本轮平台修改已保存，目标画布与操作回执已核对。' })
    if (generated.size && project) await run.append({ type: 'artifact', path: project.path, kind: 'canvas', verified: true })
    await run.state('succeeded')
    if (generated.size) options.onGenerated([...generated])
  } catch (value) {
    failed(value)
    await mutationQueue
    const unknown = Object.values(run.current.operations).some(op => op.status === 'unknown' || op.status === 'prepared')
    if (unknown) await run.state('needs_reconciliation', error?.message)
    else if (signal.aborted && !run.budgetExpired) { await run.state('cancelling'); await run.state('cancelled') }
    else await run.state('failed', error?.message)
    const details = failChatReply({ content, thinking, parts, activities: [] }, run.budgetExpired ? '运行时间预算已耗尽' : signal.aborted ? '已停止生成' : error?.message ?? '运行失败')
    parts = details.parts ?? parts; reply()
  } finally {
    clearTimeout(timeout)
    run.release()
    if (project) { registerCanvasGateway(project.canvasId); if (workspace.getSnapshot().localAgentCanvasId === project.canvasId) workspace.set({ localAgentCanvasId: null }) }
    await run.drain()
  }
  return run.current
}
