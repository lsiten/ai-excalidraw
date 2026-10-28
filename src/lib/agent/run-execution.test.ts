import { expect, it, vi } from 'vitest'
import { isTauri } from '@tauri-apps/api/core'
import { executeDrawRun } from './run-execution'
import { streamChat } from '../ai'
import { agentProjectCommand } from './project'
import type { ExcalidrawWrapperRef } from '@/components/excalidraw/wrapper'
vi.mock('@tauri-apps/api/core',()=>({isTauri:vi.fn(()=>false),invoke:vi.fn()}))
vi.mock('../ai',()=>({streamChat:vi.fn()}))
vi.mock('../workspace/store',()=>{
 const snapshot:{localAgentCanvasId:string|null}={localAgentCanvasId:null}
 return {workspace:{set:(patch:Partial<typeof snapshot>)=>Object.assign(snapshot,patch),getSnapshot:()=>snapshot,file:vi.fn(),documents:{flush:vi.fn()}}}
})
vi.mock('./project',async original=>({...await original<typeof import('./project')>(),agentProjectCommand:vi.fn()}))
it('rejects native startup when project policy denies MCP without changing source or canvas',async()=>{
 const storage=new Map<string,string>()
 vi.stubGlobal('localStorage',{getItem:(k:string)=>storage.get(k)??null,setItem:(k:string,v:string)=>storage.set(k,v)})
 // The run journal uses the browser test store; the resource-loading boundary is desktop.
 vi.mocked(isTauri).mockReturnValueOnce(false).mockReturnValueOnce(false).mockReturnValueOnce(true)
 vi.mocked(agentProjectCommand).mockResolvedValue({kind:'manifest',skills:[],mcp:[],memory:[],policy:{allow:[],deny:['mcp'],budgets:{}},provenance:{skills:'project',mcp:'project',policy:null,resolution:'project-first',fallback:null,mcpAuthority:'project'},warnings:[]})
 const addElements=vi.fn(), onReply=vi.fn()
 const canvas:ExcalidrawWrapperRef={addElements,clearCanvas:vi.fn(),getElements:()=>[],getCanvasState:()=>[],getSelectedElementsSummary:()=>[],getCanvasBounds:()=>null,
 updateElements:()=>({updated:[],notFound:[]}),deleteElements:()=>({deleted:[],notFound:[]}),scrollToElements:vi.fn(),switchToSession:vi.fn(),getCurrentSessionId:()=>null,isReady:()=>true,historyAction:()=>false,flushCanvas:async()=>true}
 const projectId=crypto.randomUUID()
 const run=await executeDrawRun({conversationId:crypto.randomUUID(),canvas,signal:new AbortController().signal,onReply,onGenerated:vi.fn(),request:{userMessage:'画一个流程图',onChunk:vi.fn(),config:{provider:'codex',apiKey:'',baseURL:'',model:''},projectContext:{projectId,canvasId:`${projectId}:test.excalidraw`,directory:'/test',path:'test.excalidraw'}}})
 expect(run.header.state).toBe('failed');expect(streamChat).not.toHaveBeenCalled();expect(addElements).not.toHaveBeenCalled()
 expect(onReply.mock.calls.at(-1)?.[0].parts).toEqual(expect.arrayContaining([expect.objectContaining({type:'status',message:expect.stringContaining('项目策略禁用了 MCP')})]))
})
