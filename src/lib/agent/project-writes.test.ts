import { beforeEach, expect, it, vi } from 'vitest'
import { RunController } from './controller'
import { agentProjectCommand } from './project'
import { writeSkillWithReceipt, writeMemoryWithReceipt } from './project-writes'
import type { AgentProjectRequest } from './project'
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>false,invoke:vi.fn()}))
vi.mock('./project',async importOriginal=>({...await importOriginal<typeof import('./project')>(),agentProjectCommand:vi.fn()}))
beforeEach(()=>{vi.clearAllMocks();const storage=new Map<string,string>();vi.stubGlobal('localStorage',{getItem:(k:string)=>storage.get(k)??null,setItem:(k:string,v:string)=>storage.set(k,v)})})
const revision=`sha256:${'a'.repeat(64)}`
const artifact={kind:'artifact' as const,artifactId:'11111111-1111-4111-8111-111111111111',path:'.agents/skills/test/SKILL.md',revision,backupPath:null,files:[],saved:true as const,verified:true as const}
const request:Extract<AgentProjectRequest,{op:'writeSkill'}>={op:'writeSkill',projectId:'project',name:'test',content:'---\nname: test\ndescription: test\n---\nRead canvas.',expectedRevision:null,authorized:true}
async function fixture(){const run=await RunController.start({backend:'api',conversationId:crypto.randomUUID(),snapshot:'{}',capabilities:[]});await run.state('running');return run}
it('reuses a saved Skill receipt and rejects changed arguments under the same call identity',async()=>{
 const run=await fixture();vi.mocked(agentProjectCommand).mockResolvedValue(artifact)
 expect(await writeSkillWithReceipt(run,request,'save')).toEqual(artifact)
 expect(await writeSkillWithReceipt(run,request,'save')).toEqual(artifact)
 await expect(writeSkillWithReceipt(run,{...request,content:'changed'},'save')).rejects.toThrow('不同 Skill')
 expect(agentProjectCommand).toHaveBeenCalledTimes(1);run.release()
})
it('retains unknown after a lost save acknowledgment and blocks a new write',async()=>{
 const run=await fixture();vi.mocked(agentProjectCommand).mockRejectedValue(new Error('connection lost'))
 await expect(writeSkillWithReceipt(run,request,'save')).rejects.toThrow('connection lost')
 expect(Object.values(run.current.operations)[0].status).toBe('unknown')
 await expect(writeSkillWithReceipt(run,request,'retry')).rejects.toThrow('核对')
 expect(agentProjectCommand).toHaveBeenCalledTimes(2);run.release()
})
it('journals only the hash of confirmed memory before invoking the native writer',async()=>{
 const run=await fixture();vi.mocked(agentProjectCommand).mockImplementation(async()=>{
  expect(Object.values(run.current.operations)[0]).toMatchObject({status:'prepared',tool:'write_memory'})
  return {...artifact,path:'.agents/memory/preference.json'}
 })
 await writeMemoryWithReceipt(run,{op:'writeMemory',projectId:'project',name:'preference',content:'偏好在画布上比较方案',expectedRevision:null,confirmed:true,authorized:true},'remember')
 const receipt=Object.values(run.current.operations)[0]
 expect(receipt.status).toBe('applied');expect(receipt.intendedAfterSnapshot).not.toContain('偏好')
 expect(JSON.parse(receipt.intendedAfterSnapshot??'{}')).toMatchObject({kind:'memory',name:'preference',contentRevision:expect.stringMatching(/^sha256:[a-f0-9]{64}$/)})
 run.release()
})
