import { z } from 'zod'
import { agentProjectCommand } from './project'
import { canvasGateway } from './canvas-gateway'
import { executeAgentTool } from './tools'
import { AgentRunError } from './store'

export const projectMcpTools = new Set(['get_project_resources', 'read_agent_file', 'write_skill', 'request_user_input', 'remember_project_fact', 'update_plan'])
export async function executeProjectMcp(name: string, input: unknown, operationId: string) {
  const { projectId, ...args } = z.object({ projectId: z.string().min(1) }).passthrough().parse(input)
  const { workspace } = await import('../workspace/store')
  const snapshot = workspace.getSnapshot()
  if (snapshot.presenting) throw new AgentRunError('请先退出演示。')
  if (snapshot.initialized === false) throw new AgentRunError('项目仍在恢复，请稍后重试。')
  const canvasId = snapshot.localAgentCanvasId
  const gateway = canvasId ? canvasGateway(canvasId) : undefined
  if (gateway && gateway.run.projectId !== projectId) throw new AgentRunError('此运行不能访问其他项目的能力。')
  if (name === 'get_project_resources') return agentProjectCommand({ op: 'manifest', projectId })
  if (name === 'read_agent_file') return agentProjectCommand({ op: 'read', projectId, ...z.object({ path: z.string() }).parse(args) })
  if (!gateway) throw new AgentRunError('该工具需要应用内运行中的任务；项目写入不能脱离用户请求。')
  gateway.run.signal.throwIfAborted()
  if (!['running','repairing'].includes(gateway.run.current.header.state)) throw new AgentRunError('任务当前不可执行工具。')
  const context = z.object({ request: z.string() }).parse(JSON.parse(gateway.run.current.header.contextSnapshot))
  return executeAgentTool({ run: gateway.run, canvas: gateway, signal: gateway.run.signal, request: context.request },
    name === 'remember_project_fact' ? 'remember_fact' : name, args, operationId)
}
