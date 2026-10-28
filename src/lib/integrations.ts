import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'

const toolIdSchema = z.enum(['codex', 'claude', 'cursor', 'pi'])
const componentStatusSchema = z.enum(['missing', 'current', 'outdated', 'blocked'])
const toolSchema = z
  .object({
    id: toolIdSchema,
    name: z.string(),
    detected: z.boolean(),
    configPath: z.string(),
    skillPath: z.string(),
    mcpStatus: componentStatusSchema,
    skillStatus: componentStatusSchema,
    note: z.string().nullish(),
  })
  .readonly()
const statusSchema = z
  .object({
    version: z.string(),
    serviceRunning: z.boolean(),
    tools: z.array(toolSchema).readonly(),
  })
  .readonly()
const resultSchema = z
  .object({
    id: toolIdSchema,
    success: z.boolean(),
    message: z.string(),
    backupPaths: z.array(z.string()).readonly(),
  })
  .readonly()
const installSchema = z.array(resultSchema).readonly()

export type IntegrationId = z.infer<typeof toolIdSchema>
export type IntegrationTool = z.infer<typeof toolSchema>
export type IntegrationStatus = z.infer<typeof statusSchema>
export type IntegrationResult = z.infer<typeof resultSchema>

export const integrationNames = {
  codex: 'Codex',
  claude: 'Claude Code',
  cursor: 'Cursor',
  pi: 'Pi',
} as const satisfies Record<IntegrationId, string>

export const integrationStatusLabels = {
  missing: '未安装',
  current: '已是最新',
  outdated: '可更新',
  blocked: '需处理',
} as const satisfies Record<z.infer<typeof componentStatusSchema>, string>

export async function getIntegrationStatus(projectId?: string): Promise<IntegrationStatus> {
  return statusSchema.parse(await (projectId ? invoke('integration_status', { projectId }) : invoke('integration_status')))
}

export async function claimIntegrationPrompt(): Promise<boolean> {
  return z.boolean().parse(await invoke('integration_claim_prompt'))
}

export async function getIntegrationGuide(projectId?: string): Promise<string> {
  const guide = z
    .object({ text: z.string().refine((text) => text.trim().length > 0) })
    .parse(await (projectId ? invoke('integration_guide', { projectId }) : invoke('integration_guide')))
  return guide.text
}

export async function installIntegrations(
  tools: readonly IntegrationId[],
  projectId?: string,
): Promise<readonly IntegrationResult[]> {
  if (tools.length === 0) return []
  const selected = [...new Set(tools)]
  const response = installSchema.parse(await invoke('integration_install', { tools: selected, ...(projectId ? { projectId } : {}) }))
  return selected.map(
    (id) =>
      response.find((result) => result.id === id) ?? {
        id,
        success: false,
        message: '未收到此工具的安装结果，请刷新状态后重试。',
        backupPaths: [],
      },
  )
}

export function integrationErrorMessage(error: unknown): string {
  if (error instanceof z.ZodError) return '应用返回的接入信息格式不正确，请重新打开应用后重试。'
  if (error instanceof Error) return error.message
  return typeof error === 'string' ? error : '操作失败，请重试。'
}
