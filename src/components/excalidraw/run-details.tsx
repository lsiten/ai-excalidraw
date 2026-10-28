import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { answerRunInput } from '@/lib/agent/tools'
import { cancelRun } from '@/lib/agent/controller'
import { runCommand, getRuns } from '@/lib/agent/store'
import { compensateCanvasRun } from '@/lib/agent/compensation'
import type { AgentRun, AgentRunState } from '@/lib/agent/protocol'
import { fileSchema } from '@/lib/workspace/types'
import { useLocale } from '@/hooks/use-locale'
import { agentProjectCommand } from '@/lib/agent/project'
import { z } from 'zod'

const labels: Record<AgentRunState, string> = {
  created:'已创建',preparing:'准备中',running:'执行中',waiting_input:'等待你的回答',validating:'校验中',repairing:'修复中',
  succeeded:'已验证完成',failed:'执行失败',cancelling:'正在停止',cancelled:'已停止',interrupted:'上次运行已中断',needs_reconciliation:'有操作待核对',
}
export function RunDetails({ run, onContinue }: { readonly run: AgentRun; readonly onContinue?: (run: AgentRun) => void }) {
  const { locale } = useLocale()
  const [value, setValue] = useState(''), [error, setError] = useState(''), [busy, setBusy] = useState(false)
  const [preview,setPreview]=useState<{path:string;content:string;revision:string}>()
  const plan = run.events.map(e => e.event).filter(e => e.type === 'plan').at(-1)
  const artifacts = run.events.map(e => e.event).filter(e => e.type === 'artifact')
  const validation = run.events.map(e => e.event).filter(e => e.type === 'validation').at(-1)
  const usage = run.events.flatMap(({event})=>event.type==='usage'?[event]:[])
  const requests = run.events.map(e => e.event).filter(e => e.type === 'input')
  const pending = requests.filter(e => !e.response && !requests.some(answer => answer.requestId === e.requestId && answer.response)).at(-1)
  const unresolved = Object.values(run.operations).filter(op => op.status === 'unknown' || op.status === 'prepared')
  const recoverable = ['interrupted','needs_reconciliation','failed','cancelled'].includes(run.header.state)
  const compensated = getRuns().some(child=>child.header.parentRunId===run.header.id && child.header.capabilities.includes('canvas_compensation') && child.header.state==='succeeded')
  const rollback = async () => {
    setBusy(true);setError('')
    try { await compensateCanvasRun(run) }
    catch (cause) { setError(cause instanceof Error?cause.message:'回退失败') }
    finally { setBusy(false) }
  }
  const previewFile=async(path:string)=>{
    if(!run.header.projectId)return
    setBusy(true);setError('')
    try{
      const file=await agentProjectCommand({op:'read',projectId:run.header.projectId,path})
      if(file.kind!=='file')throw new Error('未能读取文件。')
      setPreview(file)
    }catch(cause){setError(cause instanceof Error?cause.message:'预览失败')}
    finally{setBusy(false)}
  }
  const respond = (response: string) => {
    if (!pending) return
    try { answerRunInput(run.header.id, pending.requestId, response); setValue(''); setError('') }
    catch (cause) { setError(cause instanceof Error ? cause.message : '无法提交回答') }
  }
  const reconcile = async () => {
    setBusy(true); setError('')
    try {
      const [{ workspace }, { native }] = await Promise.all([import('@/lib/workspace/store'), import('@/lib/workspace/documents')])
      const id = run.header.canvasId
      if (!id || !run.header.projectId) throw new Error('此运行没有可核对的项目画布。')
      const file = workspace.file(id)
      const current = fileSchema.parse(await native('read_canvas_file', { projectId: file.projectId, path: file.path }))
      for (const operation of unresolved) {
        let currentRevision = current.revision
        if (operation.tool==='write_memory') {
          const intent=z.object({name:z.string()}).parse(JSON.parse(operation.intendedAfterSnapshot??'{}'))
          const manifest=await agentProjectCommand({projectId:file.projectId,op:'manifest'})
          if(manifest.kind!=='manifest')throw new Error('项目记忆状态不可读。')
          currentRevision=manifest.memory.find(fact=>fact.path===`.agents/memory/${intent.name}.json`)?.revision??'absent'
        } else if (operation.tool==='write_skill') {
          const intent=z.object({name:z.string()}).parse(JSON.parse(operation.intendedAfterSnapshot??'{}'))
          const manifest=await agentProjectCommand({projectId:file.projectId,op:'manifest'})
          const exists=manifest.kind==='manifest'&&manifest.skills.some(skill=>skill.name===intent.name&&skill.source==='project')
          if (!exists) currentRevision='absent'
          else {
            const skill=await agentProjectCommand({projectId:file.projectId,op:'read',path:`.agents/skills/${intent.name}/SKILL.md`})
            if(skill.kind!=='file')throw new Error('Skill 状态不可读。')
            currentRevision=skill.revision
          }
        }
        let settled = false
        for (const status of ['applied', 'failed'] as const) {
          try {
            await runCommand({ action:'reconcile_operation',runId:run.header.id,projectId:run.header.projectId,
              operationId:operation.operationId,status,currentRevision,expectedAfterRevision:operation.afterRevision,
            })
            settled = true; break
          } catch (cause) { if (status === 'failed') throw cause }
        }
        if (!settled) throw new Error('当前文件与操作前后状态均不一致，请手动处理冲突。')
      }
      await runCommand({action:'append',runId:run.header.id,projectId:run.header.projectId,event:{type:'validation',ok:true,message:'历史操作已按实际文件核对，未重新执行修改。可以继续剩余任务。'}})
    } catch (cause) { setError(cause instanceof Error ? cause.message : '核对失败') }
    finally { setBusy(false) }
  }
  return <div className="space-y-2 text-xs text-muted-foreground">
    <p role="status">{run.header.state==='needs_reconciliation'&&!unresolved.length?'已核对，可继续任务':locale === 'en' ? run.header.state.replaceAll('_',' ') : labels[run.header.state]}</p>
    {plan?.type === 'plan' && <ol className="list-decimal space-y-1 pl-4">{plan.steps.map((step,index) => <li key={index} className="break-words">{step}</li>)}</ol>}
    {validation?.type === 'validation' && <p>{validation.message}</p>}
    {artifacts.length > 0 && <ul className="space-y-1">{artifacts.map((artifact,index) => <li key={index} className="break-all">{artifact.verified ? '✓ ' : ''}{artifact.path}
      {['skill','memory'].includes(artifact.kind)&&run.header.projectId&&<Button variant="outline" className="mt-1 min-h-11" disabled={busy} onClick={()=>void previewFile(artifact.path)}>预览已保存文件</Button>}</li>)}</ul>}
    {preview&&<div className="min-w-0 rounded-lg border border-border p-2"><p className="break-all">{preview.path}</p><p className="break-all font-mono">{preview.revision}</p><pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all py-2">{preview.content}</pre><Button variant="outline" className="min-h-11" onClick={()=>setPreview(undefined)}>关闭预览</Button></div>}
    {run.header.state === 'waiting_input' && pending?.type === 'input' && <div className="space-y-2 rounded-lg border border-border p-3">
      <label htmlFor={`answer-${run.header.id}`} className="block whitespace-pre-wrap text-foreground">{pending.prompt}</label>
      <div className="flex flex-wrap gap-2">{pending.choices?.map(choice => <Button key={choice} variant="outline" className="min-h-11 whitespace-normal" onClick={() => respond(choice)}>{choice}</Button>)}</div>
      <Textarea id={`answer-${run.header.id}`} value={value} onChange={event => setValue(event.target.value)} placeholder={locale === 'en' ? 'Your response' : '输入你的回答'} />
      <div className="flex gap-2"><Button className="min-h-11" disabled={!value.trim()} onClick={() => respond(value)}>提交回答</Button><Button className="min-h-11" variant="outline" onClick={() => cancelRun(run.header.id)}>停止</Button></div>
    </div>}
    {recoverable && <div className="flex flex-wrap gap-2">
      {unresolved.length > 0 && <Button variant="outline" disabled={busy} onClick={() => void reconcile()}>{busy ? '核对中…' : '核对已保存的修改'}</Button>}
      {onContinue && <Button variant="outline" className="min-h-11" disabled={busy || unresolved.length > 0} onClick={() => { try { onContinue(run) } catch(cause) { setError(cause instanceof Error?cause.message:'无法继续任务') } }}>继续此任务</Button>}
    </div>}
    {!compensated && !unresolved.length && ['succeeded','failed','cancelled'].includes(run.header.state) && Object.values(run.operations).some(op=>op.status==='applied'&&['add_elements','update_elements','delete_elements'].includes(op.tool??'')) && <Button className="min-h-11" variant="outline" disabled={busy} onClick={()=>void rollback()}>撤回本轮画布修改</Button>}
    {compensated && <p>本轮画布修改已局部回退。</p>}
    {error && <p role="alert" className="text-destructive-ink">{error}</p>}
    <details><summary className="cursor-pointer py-2">运行记录</summary><p className="break-all font-mono">{run.header.id}</p>
      <p>{run.events.length} 条事件 · {Object.keys(run.operations).length} 个操作</p>
      {run.header.providerSessionId && <p className="break-all">原生会话：{run.header.providerSessionId}</p>}
      {usage.length>0 && <p>{run.header.backend==='api'?'本轮已报告用量':'服务商累计用量'}：输入 {run.header.backend==='api'?(usage.some(e=>e.inputTokens!=null)?usage.reduce((n,e)=>n+(e.inputTokens??0),0):'未知'):(usage.at(-1)?.inputTokens??'未知')} · 输出 {run.header.backend==='api'?(usage.some(e=>e.outputTokens!=null)?usage.reduce((n,e)=>n+(e.outputTokens??0),0):'未知'):(usage.at(-1)?.outputTokens??'未知')}</p>}
      <p className="whitespace-pre-wrap">{run.header.capabilities.join(' / ')}</p>
    </details>
  </div>
}
