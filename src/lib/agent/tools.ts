import { z } from 'zod'
import { isTauri } from '@tauri-apps/api/core'
import { agentProjectCommand, agentProjectRequestSchema } from './project'
import type { RunController } from './controller'
import type { CanvasGateway } from './canvas-gateway'
import { AgentRunError } from './store'
import { operationIdFor, writeSkillWithReceipt, writeMemoryWithReceipt } from './project-writes'

const answers = new Map<string, (value: string) => void>()
export function answerRunInput(runId: string, requestId: string, value: string) {
  const resolve = answers.get(`${runId}:${requestId}`)
  if (!resolve || !value.trim()) throw new AgentRunError('该输入请求已结束或尚未恢复，请继续任务后重试。')
  resolve(value.trim())
}
export type AgentTools = { readonly run: RunController; readonly canvas: Pick<CanvasGateway,'execute'>; readonly signal: AbortSignal; readonly request: string }
export function skillWriteAuthorized(request: string) {
  return /skill|技能/i.test(request) && /创建|编写|修改|优化|更新|安装|保存|create|write|update|improve|install/i.test(request)
    && !/^(?:如何|怎么|什么|请解释|介绍|说明|how\b|what\b)/i.test(request.trim())
    && !/(?:不要|禁止|勿|别|不允许|do not|don't)[^。\n]{0,20}(?:skill|技能)/i.test(request)
}
export async function executeAgentTool(context: AgentTools, name: string, args: unknown, callId: string): Promise<unknown> {
  context.signal.throwIfAborted()
  if (!['running','repairing'].includes(context.run.current.header.state)) throw new AgentRunError('任务当前不可执行工具。')
  if (name === 'update_plan') {
    const plan = z.object({ steps: z.array(z.string().max(2000)).min(1).max(32) }).parse(args)
    await context.run.append({ type: 'plan', steps: plan.steps }); return { recorded: true }
  }
  if (name === 'request_user_input' || name === 'remember_fact') {
    const input = name === 'remember_fact'
      ? z.object({ name: z.string(), content: z.string() }).parse(args)
      : z.object({ prompt: z.string().max(8000), choices: z.array(z.string()).max(10).optional() }).parse(args)
    const requestId = crypto.randomUUID()
    const prompt = 'content' in input ? `确认将以下内容保存为项目记忆：\n${input.content}` : input.prompt
    const choices = 'choices' in input ? input.choices : ['确认保存', '不保存']
    await context.run.append({ type: 'input', requestId, prompt, choices })
    await context.run.state('waiting_input')
    const key = `${context.run.id}:${requestId}`
    try {
      const response = await new Promise<string>((resolve, reject) => {
        const abort = () => { answers.delete(key); reject(context.signal.reason ?? new DOMException('Stopped', 'AbortError')) }
        answers.set(key, value => { context.signal.removeEventListener('abort', abort); resolve(value) })
        context.signal.addEventListener('abort', abort, { once: true })
        if (context.signal.aborted) abort()
      })
      await context.run.append({ type: 'input', requestId, prompt, choices, response })
      await context.run.state('running')
      if ('content' in input) {
        if (response !== '确认保存') return { saved: false, reason: '用户未确认保存' }
        if (!context.run.projectId) throw new AgentRunError('请先打开项目。')
        context.signal.throwIfAborted()
        const manifest = await agentProjectCommand({op:'manifest',projectId:context.run.projectId})
        if(manifest.kind!=='manifest')throw new AgentRunError('无法读取项目记忆清单。')
        const current=manifest.memory.find(fact=>fact.path===`.agents/memory/${input.name}.json`)
        const result = await writeMemoryWithReceipt(context.run,{ op: 'writeMemory', projectId: context.run.projectId, name: input.name, content: input.content,
          authorized: true, confirmed: true, expectedRevision: current?.revision ?? null },callId)
        if (result.kind==='artifact') await context.run.append({type:'artifact',path:result.path,kind:'memory',verified:true})
        return result
      }
      return { response }
    } finally { answers.delete(key) }
  }
  if (name === 'project_manifest' || name === 'read_skill' || name === 'write_skill') {
    if (!isTauri() || !context.run.projectId) throw new AgentRunError('项目文件工具需要桌面项目。')
    let request
    if (name === 'project_manifest') request = { op: 'manifest', projectId: context.run.projectId }
    else if (name === 'read_skill') request = { ...z.object({ path: z.string() }).parse(args), op: 'read', projectId: context.run.projectId }
    else {
      if (!skillWriteAuthorized(context.request)) {
        throw new AgentRunError('当前用户请求没有授权创建或修改 Skill。可以先返回草案。')
      }
      request = { ...z.record(z.unknown()).parse(args), op: 'writeSkill', projectId: context.run.projectId, authorized: true }
    }
    context.signal.throwIfAborted()
    const parsed = agentProjectRequestSchema.parse(request)
    const result = parsed.op==='writeSkill' ? await writeSkillWithReceipt(context.run,parsed,callId) : await agentProjectCommand(parsed)
    if (result.kind === 'artifact') await context.run.append({ type: 'artifact', path: result.path, kind: 'skill', verified: result.verified })
    return result
  }
  const operationId = await operationIdFor(context.run.id,callId)
  return context.canvas.execute(name, args, operationId)
}
