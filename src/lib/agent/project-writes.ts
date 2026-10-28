import { agentProjectCommand, projectResponseSchema, type AgentProjectRequest } from './project'
import type { RunController } from './controller'
import { AgentRunError } from './store'
import { createMemoryIntendedAfterSnapshot } from './protocol'

export async function operationIdFor(runId: string, callId: string) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`${runId}:${callId}`))).slice(0,16)
  digest[6]=(digest[6]&15)|80;digest[8]=(digest[8]&63)|128
  const hex=[...digest].map(value=>value.toString(16).padStart(2,'0')).join('')
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`
}

export async function writeMemoryWithReceipt(run: RunController, request: Extract<AgentProjectRequest,{op:'writeMemory'}>, callId: string) {
  run.signal.throwIfAborted()
  const operationId=await operationIdFor(run.id,callId), fingerprint=await revision(JSON.stringify(request))
  const previous=run.current.operations[operationId]
  if(previous?.fingerprint && previous.fingerprint!==fingerprint)throw new AgentRunError('同一操作编号不能保存不同记忆。')
  if(previous?.status==='applied'&&previous.result)return projectResponseSchema.parse(JSON.parse(previous.result))
  if(previous || Object.values(run.current.operations).some(op=>op.status==='unknown'||op.status==='prepared'))throw new AgentRunError('请先核对已有的操作。')
  const path=`.agents/memory/${request.name}.json`
  await run.transact({action:'prepare_operation',operationId,tool:'write_memory',fingerprint,targetIds:[path],baseRevision:request.expectedRevision,
    intendedAfterSnapshot:JSON.stringify(await createMemoryIntendedAfterSnapshot(request.name,request.content))})
  try {
    run.signal.throwIfAborted()
    const result=await agentProjectCommand(request)
    if(result.kind!=='artifact')throw new AgentRunError('项目记忆保存未返回文件凭据。')
    await run.transact({action:'settle_operation',operationId,status:'applied',result:JSON.stringify(result),afterRevision:result.revision})
    return result
  } catch(error) {
    let unchanged=false
    try {
      const manifest=await agentProjectCommand({op:'manifest',projectId:request.projectId})
      if(manifest.kind==='manifest'){
        const current=manifest.memory.find(fact=>fact.path===path)
        unchanged=request.expectedRevision ? current?.revision===request.expectedRevision : !current
      }
    } catch { /* Without readable evidence the result remains unknown. */ }
    await run.transact({action:'settle_operation',operationId,status:unchanged?'failed':'unknown',result:JSON.stringify({error:error instanceof Error?error.message:'记忆保存失败'})})
    throw error
  }
}
async function revision(content: string) {
  const digest = new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(content)))
  return `sha256:${[...digest].map(v=>v.toString(16).padStart(2,'0')).join('')}`
}
export async function writeSkillWithReceipt(run: RunController, request: Extract<AgentProjectRequest,{op:'writeSkill'}>, callId: string) {
  run.signal.throwIfAborted()
  const operationId = await operationIdFor(run.id,callId)
  const previous = run.current.operations[operationId]
  const fingerprint = await revision(JSON.stringify(request))
  if (previous?.fingerprint && previous.fingerprint !== fingerprint) throw new AgentRunError('同一操作编号不能用于不同 Skill 修改。')
  if (previous?.status==='applied' && previous.result) return projectResponseSchema.parse(JSON.parse(previous.result))
  if (previous) throw new AgentRunError('此 Skill 操作已经尝试过，请先读取当前版本并使用新的操作编号。')
  if (Object.values(run.current.operations).some(op=>op.status==='unknown'||op.status==='prepared')) throw new AgentRunError('请先核对结果未知的操作。')
  const prefix = `.agents/skills/${request.name}`
  const files = await Promise.all([{path:'SKILL.md',content:request.content},...(request.files??[])].map(async file=>({path:`${prefix}/${file.path}`,revision:await revision(file.content)})))
  await run.transact({action:'prepare_operation',operationId,targetIds:files.map(f=>f.path),tool:'write_skill',baseRevision:request.expectedRevision,
    fingerprint,intendedAfterSnapshot:JSON.stringify({kind:'skill',name:request.name,files}),
  })
  try {
    run.signal.throwIfAborted()
    const result = await agentProjectCommand(request)
    if (result.kind!=='artifact') throw new AgentRunError('Skill 保存未返回文件凭据。')
    await run.transact({action:'settle_operation',operationId,status:'applied',result:JSON.stringify(result),afterRevision:result.revision})
    return result
  } catch (error) {
    let unchanged = false
    try {
      if (request.expectedRevision) {
        const current = await agentProjectCommand({op:'read',projectId:request.projectId,path:`${prefix}/SKILL.md`})
        unchanged=current.kind==='file'&&current.revision===request.expectedRevision
      } else {
        const manifest = await agentProjectCommand({op:'manifest',projectId:request.projectId})
        unchanged=manifest.kind==='manifest'&&!manifest.skills.some(skill=>skill.name===request.name&&skill.source==='project')
      }
    } catch { /* A missing proof leaves the operation unknown. No write is retried. */ }
    await run.transact({action:'settle_operation',operationId,status:unchanged?'failed':'unknown',result:JSON.stringify({error:error instanceof Error?error.message:'Skill 保存失败'})})
    throw error
  }
}
