import { z } from 'zod'
import { RunController } from './controller'
import { CanvasGateway } from './canvas-gateway'
import type { AgentRun, OperationReceipt } from './protocol'
import { canvasCommandSchema, streamElementPatchSchema, type CanvasCommand } from '../canvas-commands'
import { AgentRunError } from './store'

export function inverseCanvasOperation(receipt: OperationReceipt): CanvasCommand[] {
  if (!receipt.intendedAfterSnapshot || !receipt.beforeSnapshot) throw new AgentRunError('操作缺少可验证的回退快照。')
  const command = canvasCommandSchema.parse(JSON.parse(receipt.intendedAfterSnapshot))
  const before = z.array(z.record(z.unknown())).parse(JSON.parse(receipt.beforeSnapshot))
  const previous = (id: string) => {
    const element = before.find(e => e.id === id)
    if (!element) throw new AgentRunError(`缺少元素 ${id} 的修改前状态。`)
    return streamElementPatchSchema.parse(element)
  }
  switch (command.tool) {
    case 'add_elements': return [{ tool:'delete_elements',arguments:{ids:command.arguments.elements.map(e=>e.id)} }]
    case 'update_elements': return [{tool:'update_elements',arguments:{elements:command.arguments.elements.map(e=>previous(e.id))}}]
    case 'delete_elements': {
      const result = z.object({deleted:z.array(z.string())}).parse(JSON.parse(receipt.result ?? '{}'))
      const restored = result.deleted.map(previous).sort((a,b)=>Number(a.type==='text')-Number(b.type==='text'))
      return restored.map(element => canvasCommandSchema.parse({tool:'add_elements',arguments:{elements:[element]}}))
    }
    case 'undo':
    case 'redo':
    case 'get_canvas':
    case 'fit_canvas': throw new AgentRunError('此操作没有安全的局部回退方式。')
  }
}

export async function compensateCanvasRun(original: AgentRun) {
  const { workspace } = await import('../workspace/store')
  const canvasId = original.header.canvasId
  if (!canvasId || !original.header.projectId) throw new AgentRunError('回退需要打开原项目画布。')
  const operations = original.events.flatMap(({event})=>event.type==='operation' && event.status==='prepared' && original.operations[event.operationId]?.status==='applied' && ['add_elements','update_elements','delete_elements'].includes(original.operations[event.operationId]?.tool??'')
    ? [original.operations[event.operationId]] : [])
  const last = operations.at(-1)
  if (!last?.afterRevision) throw new AgentRunError('没有可安全回退的画布操作。')
  const editor = await workspace.documents.ready(canvasId)
  if(!await editor.flushCanvas())throw new AgentRunError('画布尚未就绪。'); await workspace.documents.flush(canvasId)
  if (workspace.file(canvasId).revision !== last.afterRevision) throw new AgentRunError('画布在本轮之后已变化，不能覆盖后续修改。请手动处理。')
  const file = workspace.file(canvasId), project = workspace.getSnapshot().projects.find(p=>p.id===file.projectId)
  if (!project) throw new AgentRunError('原项目未打开。')
  const inverses = operations.reverse().flatMap(inverseCanvasOperation)
  return workspace.run(async () => {
    const run = await RunController.start({conversationId:original.header.conversationId,backend:original.header.backend,parentRunId:original.header.id,
      project:{projectId:file.projectId,canvasId,path:file.path,directory:project.directory},snapshot:JSON.stringify({request:'撤回本轮平台画布修改',compensates:original.header.id}),capabilities:['canvas_compensation'],
    })
    const gateway = new CanvasGateway(run,{editor,revision:()=>workspace.file(canvasId).revision,flush:async()=>{if(!await editor.flushCanvas())throw new AgentRunError('保存失败。');await workspace.documents.flush(canvasId)}})
    try {
      await run.state('running')
      for (const command of inverses) await gateway.execute(command.tool,command.arguments)
      await run.state('validating');await run.append({type:'validation',ok:true,message:'已按本轮操作快照局部回退，未调用全局撤销。'});await run.state('succeeded')
    } catch (error) {
      await run.state(Object.values(run.current.operations).some(op=>op.status==='unknown')?'needs_reconciliation':'failed',error instanceof Error?error.message:'回退失败')
      throw error
    } finally { run.release() }
    return run.current
  })
}
