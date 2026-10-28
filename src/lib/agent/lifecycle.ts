import type { AgentRunState } from './protocol'
import { AgentRunError } from './errors'
const next: Readonly<Record<AgentRunState, readonly AgentRunState[]>> = {
  created:['preparing'],preparing:['running'],running:['waiting_input','validating'],waiting_input:['running'],
  validating:['succeeded','repairing'],repairing:['running','validating'],cancelling:['cancelled'],
  interrupted:['failed','cancelled'],needs_reconciliation:['failed','cancelled'],succeeded:[],failed:[],cancelled:[],
}
export function assertRunTransition(from: AgentRunState, to: AgentRunState) {
  if (['succeeded','failed','cancelled'].includes(from)) throw new AgentRunError('已结束的运行不能继续变化。')
  if (!next[from].includes(to) && !['failed','cancelling','interrupted','needs_reconciliation'].includes(to)) throw new AgentRunError(`无效运行状态变化：${from} → ${to}`)
}
