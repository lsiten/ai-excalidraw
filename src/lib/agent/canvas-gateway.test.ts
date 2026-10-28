import { beforeEach, expect, it, vi } from 'vitest'
import { RunController } from './controller'
import { CanvasGateway } from './canvas-gateway'
import type { CanvasCommandTarget } from '../canvas-commands'
import type { ParsedElement } from '@/components/excalidraw/element-parser'
vi.mock('@tauri-apps/api/core', () => ({ isTauri:()=>false,invoke:vi.fn() }))
beforeEach(() => {
  const storage = new Map<string,string>()
  vi.stubGlobal('localStorage',{getItem:(key:string)=>storage.get(key)??null,setItem:(key:string,value:string)=>storage.set(key,value)})
})
async function fixture(failSave = false) {
  const run = await RunController.start({conversationId:crypto.randomUUID(),backend:'api',snapshot:'{}',capabilities:[]})
  await run.state('running')
  let revision = 'before'
  const scene: (ParsedElement & {type:string})[] = []
  const addElements = vi.fn((elements:ParsedElement[])=>{scene.push(...elements.map(e=>({...e,type:e.type??'rectangle'})))})
  const editor: CanvasCommandTarget = {
    getElements:()=>scene,addElements,updateElements:elements=>{for(const patch of elements){const e=scene.find(e=>e.id===patch.id);if(e)Object.assign(e,patch)}return{updated:elements.map(e=>e.id),notFound:[]}},
    deleteElements:ids=>{for(const id of ids){const i=scene.findIndex(e=>e.id===id);if(i>=0)scene.splice(i,1)}return{deleted:ids,notFound:[]}},
    getCurrentSessionId:()=>null,getSelectedElementsSummary:()=>[],getCanvasBounds:()=>null,scrollToElements:()=>undefined,historyAction:()=>false,
  }
  const gateway = new CanvasGateway(run,{editor,revision:()=>revision,flush:async()=>{if(failSave)throw new Error('disk unavailable');revision='after'}})
  return {run,gateway,scene,addElements}
}
const shape = {id:'box',type:'rectangle',x:0,y:0,width:100,height:80}
it('replays an applied receipt without creating the same shape twice', async () => {
  // Given a saved operation.
  const f = await fixture(), id = crypto.randomUUID()
  const first = await f.gateway.execute('add_elements',{elements:[shape]},id)
  // When the same operation ID is delivered again.
  const second = await f.gateway.execute('add_elements',{elements:[shape]},id)
  // Then the stored result returns without another mutation.
  expect(second).toEqual(first);expect(f.addElements).toHaveBeenCalledTimes(1)
  expect(f.run.current.operations[id]).toMatchObject({status:'applied',baseRevision:'before',afterRevision:'after'})
  f.run.release()
})
it('preserves selected element identity when applying a streamed partial update', async () => {
  // Given an existing shape.
  const f = await fixture();await f.gateway.apply([shape])
  // When the model streams only the changed property.
  await f.gateway.apply([{id:'box',backgroundColor:'#dbeafe'}])
  // Then the original ID, geometry and other elements survive.
  expect(f.scene).toHaveLength(1);expect(f.scene[0]).toMatchObject({...shape,backgroundColor:'#dbeafe'})
  f.run.release()
})
it('marks uncertain writes unknown and prevents further mutations after save failure', async () => {
  // Given a mutation whose persistence fails.
  const f = await fixture(true), id = crypto.randomUUID()
  // When its result cannot be confirmed.
  await expect(f.gateway.execute('add_elements',{elements:[shape]},id)).rejects.toThrow('disk unavailable')
  // Then it cannot be blindly repeated under a different operation ID.
  expect(f.run.current.operations[id]?.status).toBe('unknown')
  await expect(f.gateway.execute('add_elements',{elements:[{...shape,id:'another'}]})).rejects.toThrow('未知')
  expect(f.addElements).toHaveBeenCalledTimes(1);f.run.release()
})
