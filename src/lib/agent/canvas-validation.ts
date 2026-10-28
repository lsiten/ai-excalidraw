import { z } from 'zod'
import type { CanvasCommand } from '../canvas-commands'
import { AgentRunError } from './errors'
export function verifyCanvasEffect(command: CanvasCommand, input: unknown) {
  const elements=z.array(z.record(z.unknown())).parse(input)
  const current=new Map(elements.filter(e=>e.isDeleted!==true).map(e=>[e.id,e]))
  if(command.tool==='delete_elements'){
    if(command.arguments.ids.some(id=>current.has(id)))throw new AgentRunError('删除结果未在画布中生效。')
    return
  }
  if(command.tool!=='add_elements'&&command.tool!=='update_elements')return
  for(const patch of command.arguments.elements){
    const actual=current.get(patch.id)
    if(!actual)throw new AgentRunError(`画布中缺少目标元素：${patch.id}`)
    for(const [key,value] of Object.entries(patch)){
      const received=actual[key]
      const equal=typeof value==='number'&&typeof received==='number'?Math.abs(value-received)<0.000001:JSON.stringify(value)===JSON.stringify(received)
      if(!equal)throw new AgentRunError(`元素 ${patch.id} 的 ${key} 未达到请求值。`)
    }
  }
}
