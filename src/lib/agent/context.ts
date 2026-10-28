import { z } from 'zod'
import { getRuns, AgentRunError } from './store'
import type { AgentRun } from './protocol'
import type { agentProjectCommand } from './project'

const snapshotSchema = z.object({ request: z.string(), project: z.object({ canvasId: z.string() }).optional() })
type Manifest = Extract<Awaited<ReturnType<typeof agentProjectCommand>>, {kind:'manifest'}>
export function summarizeProjectResources(manifest: Manifest, request: string) {
  const skills=[...manifest.skills].sort((a,b)=>Number(request.includes(b.name))-Number(request.includes(a.name)))
  return { ...manifest,
    skills:skills.slice(0,32).map(({name,description,path,revision,source})=>({name,description:description.slice(0,160),path,revision,source})),
    memory:manifest.memory.slice(0,8).map(fact=>({...fact,summary:fact.summary.slice(0,256)})),
    availableSkills:manifest.skills.length,availableFacts:manifest.memory.length,
    contextTruncated:manifest.skills.length>32||manifest.memory.length>8,
  }
}
export function continuationPrompt(run: AgentRun) {
  if (Object.values(run.operations).some(op => op.status === 'unknown' || op.status === 'prepared')) throw new AgentRunError('请先核对未确认的操作，再继续任务。')
  const snapshot = snapshotSchema.parse(JSON.parse(run.header.contextSnapshot))
  const completed = Object.values(run.operations).filter(op => op.status === 'applied').map(op => ({ tool: op.tool, targetIds: op.targetIds }))
  return `继续完成上次任务：${snapshot.request}\n已确认的平台操作：${JSON.stringify(completed)}。先读取最新画布和项目文件，再执行剩余步骤；不要重复已保存的修改。`
}
export function selectedArtifactContext(canvasId: string | undefined, selectedIds: readonly string[]) {
  if (!canvasId || selectedIds.length === 0) return []
  const selected = new Set(selectedIds)
  return getRuns().filter(run => run.header.canvasId === canvasId).flatMap(run =>
    Object.values(run.operations).filter(op => op.targetIds.some(id => selected.has(id))).map(op => ({
      runId: run.header.id, operationId: op.operationId, status: op.status, elementIds: op.targetIds,
    })),
  ).slice(-20)
}
