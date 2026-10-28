import { beforeEach, expect, it, vi } from 'vitest'
import { RunController, cancelRun } from './controller'
import { loadRuns, runCommand } from './store'
import { executeAgentTool, answerRunInput, skillWriteAuthorized } from './tools'
import { replayRun } from './replay'
import { assertCanvasOutputComplete } from '@/components/excalidraw/element-parser'
import { boundedAgentText } from './text'
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>false,invoke:vi.fn()}))
beforeEach(()=>{
 const values=new Map<string,string>()
 vi.stubGlobal('localStorage',{getItem:(key:string)=>values.get(key)??null,setItem:(key:string,value:string)=>values.set(key,value)})
})
async function fixture(){const run=await RunController.start({backend:'api',conversationId:crypto.randomUUID(),snapshot:'{}',capabilities:[]});await run.state('running');return run}
it('coalesces text without reordering message, tool and state events',async()=>{
 const run=await fixture()
 run.stream({type:'message',text:'甲'});run.stream({type:'message',text:'乙'})
 await run.append({type:'tool',callId:'read',name:'get_canvas',status:'completed'})
 run.stream({type:'message',text:'丙'});await run.state('validating');await run.state('succeeded');await run.drain()
 expect(replayRun(run.current).filter(p=>p.type==='chunk').map(p=>p.text)).toEqual(['甲乙','丙'])
 expect(run.current.events.map(e=>e.seq)).toEqual(run.current.events.map((_,i)=>i+1))
 await expect(run.state('running')).rejects.toThrow('结束')
 run.release()
})
it('waits for an actual input response and rejects responses after completion',async()=>{
 const run=await fixture(), gateway={execute:vi.fn()}
 const promise=executeAgentTool({run,canvas:gateway,signal:run.signal,request:'确认偏好'},'request_user_input',{prompt:'选择配色',choices:['蓝色','绿色']},'question')
 await vi.waitFor(()=>expect(run.current.header.state).toBe('waiting_input'))
 const event=run.current.events.map(e=>e.event).find(e=>e.type==='input')
 if(event?.type!=='input')throw new Error('missing question')
 answerRunInput(run.id,event.requestId,'绿色')
 expect(await promise).toEqual({response:'绿色'});expect(run.current.header.state).toBe('running')
 expect(()=>answerRunInput(run.id,event.requestId,'蓝色')).toThrow('结束')
 run.release()
})
it('cancels a projectless run while it waits for the human',async()=>{
 const run=await fixture(), gateway={execute:vi.fn()}
 const promise=executeAgentTool({run,canvas:gateway,signal:run.signal,request:'问我'},'request_user_input',{prompt:'继续？'},'question')
 const rejected=expect(promise).rejects.toThrow('停止')
 await vi.waitFor(()=>expect(run.current.header.state).toBe('waiting_input'))
 cancelRun(run.id);await rejected;expect(run.signal.aborted).toBe(true);run.release()
})
it('recovers an orphaned prepared operation as unknown without replaying effects',async()=>{
 const run=await fixture(), operationId=crypto.randomUUID()
 await run.transact({action:'prepare_operation',operationId,targetIds:['box'],tool:'add_elements'})
 run.release();await loadRuns()
 const restored=await runCommand({action:'read',runId:run.id})
 if(Array.isArray(restored))throw new Error('invalid run')
 expect(restored.header.state).toBe('needs_reconciliation');expect(restored.operations[operationId].status).toBe('unknown')
 await expect(runCommand({action:'reconcile_operation',runId:run.id,operationId,status:'applied'})).rejects.toThrow('实际文件')
})
it('requires explicit Skill write intent and preserves explanation-only requests',()=>{
 expect(skillWriteAuthorized('创建一个能结合画布反馈的 Skill')).toBe(true)
 for(const request of ['如何创建 Skill','画一个流程图','不要安装 Skill，只解释步骤',"don't update the skill"])expect(skillWriteAuthorized(request)).toBe(false)
})
it('rejects truncated canvas data while excluding fenced documentation',()=>{
 expect(()=>assertCanvasOutputComplete('<canvas>{"id":"a","x":1}</canvas>')).not.toThrow()
 expect(()=>assertCanvasOutputComplete('<canvas>{"id":"a","x":')).toThrow('结束')
 expect(()=>assertCanvasOutputComplete('<canvas>{"id":"a",}</canvas>')).toThrow()
 expect(()=>assertCanvasOutputComplete('```xml\n<canvas>{"id":"example"\n```')).not.toThrow()
})
it('bounds large UTF-8 tool details without corrupting code points',()=>{
 const text=boundedAgentText('画布反馈🟦'.repeat(16000))
 expect(new TextEncoder().encode(text).length).toBeLessThanOrEqual(60000)
 expect(text).not.toContain('\ufffd');expect(text).toContain('已截断')
})
