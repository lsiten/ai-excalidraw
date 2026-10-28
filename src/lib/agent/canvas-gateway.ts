import { z } from 'zod'
import { canvasCommandSchema, executeCanvasCommand, streamElementPatchSchema, type CanvasCommandTarget } from '../canvas-commands'
import type { ParsedElement } from '@/components/excalidraw/element-parser'
import type { RunController } from './controller'
import { AgentRunError } from './store'
import { verifyCanvasEffect } from './canvas-validation'

export type CanvasAccess = {
  readonly editor: CanvasCommandTarget
  readonly revision: () => string
  readonly flush: () => Promise<void>
}
const gateways = new Map<string, CanvasGateway>()
export const canvasGateway = (id: string) => gateways.get(id)
export const registerCanvasGateway = (id: string, gateway?: CanvasGateway) => { if (gateway) gateways.set(id, gateway); else gateways.delete(id) }
const resultSchema = z.object({ notFound: z.array(z.string()).optional() }).passthrough()
async function fingerprint(value: string) {
  const data = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value))
  return [...new Uint8Array(data)].map(v => v.toString(16).padStart(2, '0')).join('')
}

export class CanvasGateway {
  private pending: Promise<void> = Promise.resolve()
  readonly run: RunController
  private readonly access: CanvasAccess
  constructor(run: RunController, access: CanvasAccess) { this.run = run; this.access = access }
  execute(name: string, args: unknown, operationId: string = crypto.randomUUID()): Promise<unknown> {
    const command = canvasCommandSchema.parse({ tool: name === 'get_canvas_elements' ? 'get_canvas' : name, arguments: args })
    if (command.tool === 'get_canvas') return Promise.resolve(executeCanvasCommand(this.access.editor, command))
    if (command.tool === 'fit_canvas') return Promise.resolve(executeCanvasCommand(this.access.editor, command))
    const operation = this.pending.then(async () => {
      this.run.signal.throwIfAborted()
      if (Object.values(this.run.current.operations).some(op => op.status === 'unknown')) throw new AgentRunError('存在结果未知的操作，请先核对再继续。')
      const previous = this.run.current.operations[operationId]
      const commandFingerprint = await fingerprint(JSON.stringify(command))
      if (previous?.fingerprint && previous.fingerprint !== commandFingerprint) throw new AgentRunError('同一操作编号不能用于不同修改。')
      if (previous?.status === 'applied') return previous.result ? JSON.parse(previous.result) : { applied: true }
      if (previous) throw new AgentRunError('该操作已经尝试过，请使用新操作编号并读取最新画布。')
      const beforeRevision = this.access.revision()
      const before = JSON.stringify(this.access.editor.getElements())
      const targetIds = command.tool === 'delete_elements' ? command.arguments.ids
        : command.tool === 'add_elements' || command.tool === 'update_elements' ? command.arguments.elements.map(e => e.id) : []
      await this.run.transact({ action: 'prepare_operation', operationId, targetIds,
        tool: command.tool, fingerprint: commandFingerprint, baseRevision: beforeRevision,
        beforeSnapshot: new TextEncoder().encode(before).length <= 60000 ? before : undefined,
        intendedAfterSnapshot: JSON.stringify(command),
      })
      if (beforeRevision !== this.access.revision() || before !== JSON.stringify(this.access.editor.getElements())) {
        await this.run.transact({ action: 'settle_operation', operationId, status: 'failed', result: JSON.stringify({ conflict: true }) })
        throw new AgentRunError('画布在操作前已变化，请读取最新内容后重试。')
      }
      let touched = false
      try {
        this.run.signal.throwIfAborted()
        if (command.tool === 'update_elements' || command.tool === 'delete_elements') {
          const existing = new Set(this.access.editor.getElements().map(e => e.id))
          if (targetIds.some(id => !existing.has(id))) throw new AgentRunError('目标元素不存在，请读取最新画布。')
        }
        touched = true
        const result = executeCanvasCommand(this.access.editor, command)
        verifyCanvasEffect(command,this.access.editor.getElements())
        const parsed = resultSchema.safeParse(result)
        if (parsed.success && parsed.data.notFound?.length) throw new AgentRunError(`目标元素不存在：${parsed.data.notFound.join(', ')}`)
        await this.access.flush()
        const after = JSON.stringify(this.access.editor.getElements())
        await this.run.transact({ action: 'settle_operation', operationId, status: 'applied',
          result: JSON.stringify(result), afterRevision: this.access.revision(), afterSnapshot: new TextEncoder().encode(after).length <= 60000 ? after : undefined,
        })
        return result
      } catch (error) {
        await this.run.transact({ action: 'settle_operation', operationId, status: touched ? 'unknown' : 'failed',
          result: JSON.stringify({ error: error instanceof Error ? error.message : '画布操作失败' }),
        })
        throw error
      }
    })
    this.pending = operation.then(() => undefined, () => undefined)
    return operation
  }
  async apply(elements: readonly ParsedElement[]) {
    const visibleIds = new Set<string>()
    for (const element of elements) {
      if (element.isDeleted) { await this.execute('delete_elements', { ids: [element.id] }); continue }
      const existing = this.access.editor.getElements().some(e => e.id === element.id)
      await this.execute(existing ? 'update_elements' : 'add_elements', { elements: [streamElementPatchSchema.parse(element)] })
      visibleIds.add(element.id)
    }
    return [...visibleIds]
  }
  async drain() { await this.pending; await this.run.drain() }
}
