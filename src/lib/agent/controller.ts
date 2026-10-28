import type { AIProvider } from '../ai-config'
import type { AIProjectContext, LocalChatActivity } from '../local-chat'
import { runCommand, publishRun, AgentRunError, releaseRunOwnership } from './store'
import type { AgentRun, AgentRunEvent, AgentRunState, AgentRunRequest } from './protocol'

export type RunContext = {
  readonly conversationId: string
  readonly backend: AIProvider
  readonly project?: AIProjectContext
  readonly parentRunId?: string
  readonly snapshot: string
  readonly capabilities: readonly string[]
}
const active = new Map<string, RunController>()
const byId = new Map<string, RunController>()
const reservations = new Set<string>()
export function activeCanvasRun(canvasId: string) { return active.get(canvasId) }
export function cancelRun(runId: string) { byId.get(runId)?.cancel() }

export class RunController {
  private record: AgentRun
  private pending: Promise<void> = Promise.resolve()
  private storageFailure: Error | undefined
  private readonly cancellation = new AbortController()
  private stoppedByBudget = false
  private buffered: Extract<AgentRunEvent,{type:'message'|'thinking'}> | undefined
  private flushTimer: ReturnType<typeof setTimeout> | undefined
  private detach: (() => void) | undefined
  private constructor(record: AgentRun) { this.record = record }
  get id() { return this.record.header.id }
  get current() { return this.record }
  get projectId() { return this.record.header.projectId ?? undefined }
  get signal() { return this.cancellation.signal }
  get budgetExpired() { return this.stoppedByBudget }
  cancel(reason: 'user' | 'budget' = 'user') {
    if (this.signal.aborted) return
    this.stoppedByBudget = reason === 'budget'
    this.cancellation.abort(new DOMException(this.stoppedByBudget ? '运行时间预算已耗尽' : '已停止运行', 'AbortError'))
  }
  bind(parent: AbortSignal) {
    const stop = () => this.cancel()
    parent.addEventListener('abort', stop, { once: true })
    this.detach = () => parent.removeEventListener('abort', stop)
    if (parent.aborted) stop()
  }
  static async start(context: RunContext) {
    if(new TextEncoder().encode(context.snapshot).length>64000)throw new AgentRunError('任务上下文过大，请缩小选区或拆分请求。')
    const canvasId = context.project?.canvasId
    if (canvasId && (active.has(canvasId) || reservations.has(canvasId))) throw new AgentRunError('此画布已有运行中的任务。')
    if (canvasId) reservations.add(canvasId)
    try {
    const result = await runCommand({ action: 'create', header: {
      id: crypto.randomUUID(), conversationId: context.conversationId, backend: context.backend,
      parentRunId: context.parentRunId, projectId: context.project?.projectId, canvasId: context.project?.canvasId,
      contextSnapshot: context.snapshot, capabilities: [...context.capabilities],
      state: 'created', createdAt: Date.now(), updatedAt: Date.now(),
    } })
    if (Array.isArray(result)) throw new AgentRunError('运行存储返回无效结果。')
    const controller = new RunController(result)
    try { await controller.state('preparing') } catch(error) { controller.release(); throw error }
    byId.set(controller.id, controller)
    if (context.project) active.set(context.project.canvasId, controller)
    return controller
    } finally { if (canvasId) reservations.delete(canvasId) }
  }
  private enqueue(event: AgentRunEvent): Promise<void> {
    this.pending = this.pending.then(async () => {
      if (this.storageFailure) throw this.storageFailure
      const result = await runCommand({ action: 'append', runId: this.id, projectId: this.projectId, event })
      if (Array.isArray(result)) throw new AgentRunError('运行存储返回无效结果。')
      this.record = result
    }).catch((error: unknown) => {
      this.storageFailure = error instanceof Error ? error : new AgentRunError('运行记录保存失败。')
    })
    return this.pending.then(() => { if(this.storageFailure)throw this.storageFailure })
  }
  private flushBuffered() {
    clearTimeout(this.flushTimer)
    this.flushTimer=undefined
    const event=this.buffered;this.buffered=undefined
    if(event)void this.enqueue(event).catch(error=>{this.storageFailure=error instanceof Error?error:new AgentRunError('运行记录保存失败')})
  }
  stream(event: Extract<AgentRunEvent,{type:'message'|'thinking'}>) {
    if(this.signal.aborted)return
    if(event.text.length>12000){
      for(let start=0;start<event.text.length;){let end=Math.min(start+12000,event.text.length);const c=event.text.charCodeAt(end-1);if(c>=0xd800&&c<=0xdbff)end-=1;this.stream({...event,text:event.text.slice(start,end)});start=end}
      return
    }
    const previous=this.buffered
    if(previous?.type===event.type&&previous.text.length+event.text.length<16000&&
      (previous.type!=='message'||event.type!=='message'||previous.phase===event.phase))this.buffered={...event,text:previous.text+event.text}
    else {this.flushBuffered();this.buffered=event}
    this.flushTimer??=setTimeout(()=>{this.flushTimer=undefined;this.flushBuffered()},40)
  }
  append(event: AgentRunEvent) { this.flushBuffered();return this.enqueue(event) }
  async drain() { this.flushBuffered();await this.pending; if (this.storageFailure) throw this.storageFailure }
  state(state: AgentRunState, message?: string) { return this.append({ type: 'state', state, message }) }
  activity(activity: LocalChatActivity) {
    return this.append(activity.type === 'status'
      ? { type: 'status', message: activity.message, level: activity.level ?? 'info' }
      : { type: 'tool', callId: activity.id, name: activity.name, status: activity.status, input: activity.input, output: activity.output })
  }
  async transact(request: Omit<Extract<AgentRunRequest, { action: 'prepare_operation' }>, 'runId' | 'projectId'> | Omit<Extract<AgentRunRequest, { action: 'settle_operation' }>, 'runId' | 'projectId'>) {
    this.flushBuffered()
    const transaction = this.pending.then(async () => {
      if (this.storageFailure) throw this.storageFailure
      const result = await runCommand({ ...request, runId: this.id, projectId: this.projectId })
      if (Array.isArray(result)) throw new AgentRunError('操作回执无效。')
      this.record = publishRun(result)
      return result
    })
    this.pending = transaction.then(() => undefined, () => undefined)
    const result = await transaction
    return result.operations[request.operationId]
  }
  release() {
    this.flushBuffered()
    releaseRunOwnership(this.id)
    byId.delete(this.id)
    this.cancellation.abort(new DOMException('运行已结束', 'AbortError'))
    this.detach?.()
    const canvasId = this.record.header.canvasId
    if (canvasId && active.get(canvasId) === this) active.delete(canvasId)
  }
}
