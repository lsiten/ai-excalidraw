import { isTauri, invoke } from '@tauri-apps/api/core'
import { AgentRunSchema, agentRunCommand, type AgentRun, type AgentRunRequest } from './protocol'
import { assertRunTransition } from './lifecycle'
import { AgentRunError } from './errors'
export { AgentRunError } from './errors'

const records = new Map<string, AgentRun>()
const owned = new Set<string>()
const listeners = new Set<() => void>()
let snapshot: readonly AgentRun[] = []
export const subscribeRuns = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } }
export const getRuns = () => snapshot
export const releaseRunOwnership = (id: string) => owned.delete(id)
export function publishRun(run: AgentRun): AgentRun {
  records.set(run.header.id, run)
  snapshot = [...records.values()].sort((a, b) => b.header.createdAt - a.header.createdAt)
  listeners.forEach(listener => listener())
  return run
}

// Web keeps the same event protocol; desktop records are owned by the native store.
async function webCommand(request: AgentRunRequest): Promise<AgentRun | AgentRun[]> {
  const key = `ai-agent-runs:${(request.action === 'create' ? request.header.projectId : request.projectId) ?? 'local'}`
  const loaded: unknown = JSON.parse(localStorage.getItem(key) ?? '[]')
  const runs = AgentRunSchema.array().parse(loaded)
  if (request.action === 'list') return runs
  if (request.action === 'create') {
    if(request.header.state!=='created'||runs.some(r=>r.header.id===request.header.id))throw new AgentRunError('运行创建状态或编号无效。')
    const run = AgentRunSchema.parse({ header: request.header, events: [], operations: {} })
    runs.push(run); localStorage.setItem(key, JSON.stringify(runs)); return run
  }
  const run = runs.find(r => r.header.id === request.runId)
  if (!run) throw new AgentRunError('未找到运行记录。')
  if (request.action === 'read') return run
  if (request.action === 'append') {
    if (['succeeded','failed','cancelled'].includes(run.header.state))throw new AgentRunError('运行已经结束。')
    if (request.event.type === 'state') {
      assertRunTransition(run.header.state,request.event.state)
      if(request.event.state==='succeeded'&&Object.values(run.operations).some(op=>op.status==='prepared'||op.status==='unknown'))throw new AgentRunError('仍有未核对的操作。')
      run.header.state = request.event.state
    }
    if(request.event.type==='context'){
      if(run.header.state!=='preparing')throw new AgentRunError('上下文已经固定。')
      run.header.contextSnapshot=request.event.snapshot;run.header.capabilities=request.event.capabilities
    }
    if(request.event.type==='session'){
      if(request.event.backend!==run.header.backend)throw new AgentRunError('会话来源不匹配。')
      run.header.providerSessionId=request.event.sessionId
    }
    run.events.push({ runId: run.header.id, seq: run.events.length + 1, at: Date.now(), event: request.event })
  } else if (request.action === 'prepare_operation') {
    if(!['running','repairing','validating'].includes(run.header.state))throw new AgentRunError('运行当前不可修改画布。')
    const existing = run.operations[request.operationId]
    if(existing && (existing.fingerprint!==request.fingerprint || JSON.stringify(existing.targetIds)!==JSON.stringify(request.targetIds)))throw new AgentRunError('操作编号冲突。')
    if (!existing) {
      run.operations[request.operationId] = { ...request, operationId: request.operationId, status: 'prepared', targetIds: request.targetIds }
      run.events.push({runId:run.header.id,seq:run.events.length+1,at:Date.now(),event:{type:'operation',operationId:request.operationId,status:'prepared',targetIds:request.targetIds}})
    }
  } else if (request.action === 'settle_operation') {
    const operation = run.operations[request.operationId]
    if (!operation) throw new AgentRunError('未找到操作回执。')
    if(operation.status!=='prepared' && !(operation.status===request.status&&operation.result===request.result))throw new AgentRunError('操作状态不可重复修改。')
    Object.assign(operation, request)
    run.events.push({runId:run.header.id,seq:run.events.length+1,at:Date.now(),event:{type:'operation',operationId:request.operationId,status:request.status,targetIds:operation.targetIds}})
  } else if (request.action === 'reconcile_operation') {
    throw new AgentRunError('操作核对需要桌面项目的实际文件，网页无法提供保存凭据。')
  }
  run.header.updatedAt = Date.now()
  localStorage.setItem(key, JSON.stringify(runs))
  return AgentRunSchema.parse(run)
}

export async function runCommand(request: AgentRunRequest) {
  const result = await (isTauri() ? agentRunCommand(request) : webCommand(request))
  if (Array.isArray(result)) { result.forEach(publishRun); return result }
  if (request.action === 'create') owned.add(result.header.id)
  return publishRun(result)
}
export async function loadRuns(projectId?: string) {
  const result = await runCommand({ action: 'list', projectId })
  const runs = Array.isArray(result) ? result : [result]
  for (const run of runs) {
    if (owned.has(run.header.id) || ['succeeded','failed','cancelled','interrupted','needs_reconciliation'].includes(run.header.state)) continue
    if (isTauri()) await invoke('local_chat_cancel', { requestId: run.header.id })
    for (const operation of Object.values(run.operations)) {
      if (operation.status === 'prepared') await runCommand({ action: 'settle_operation', projectId, runId: run.header.id, operationId: operation.operationId, status: 'unknown' })
    }
    const pending = Object.values(run.operations).some(operation => operation.status === 'prepared' || operation.status === 'unknown')
    await runCommand({ action: 'append', projectId, runId: run.header.id,
      event: { type: 'state', state: pending ? 'needs_reconciliation' : 'interrupted', message: '界面已恢复；先核对上次运行，未重新执行任何操作。' },
    })
  }
  return runs
}
