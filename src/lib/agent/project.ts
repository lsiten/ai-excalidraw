import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'

export const capabilitySchema = z.enum(['read', 'writeSkill', 'writeMemory', 'writePolicy', 'mcp'])
export const projectBudgetsSchema = z.object({
  maxTurns: z.number().int().nonnegative().max(0xffffffff).nullable().optional(),
  repairLimit: z.number().int().nonnegative().max(0xffffffff).nullable().optional(),
  timeoutMs: z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER).nullable().optional(),
}).strict()
export const projectPolicySchema = z.object({
  allow: z.array(capabilitySchema).default([]),
  deny: z.array(capabilitySchema).default([]),
  budgets: projectBudgetsSchema.default({}),
}).strict()
const revision = z.string().regex(/^sha256:[a-f0-9]{64}$/)
const write = { authorized: z.literal(true), expectedRevision: revision.nullable() }
const name = z.string().min(1).max(64).regex(/^[a-z0-9]+(?:-[a-z0-9]+)*$/)
const content = z.string().refine(value => new TextEncoder().encode(value).length <= 262144, 'File exceeds 256 KiB')
const resourcePath = z.string().min(1).max(1024).refine(value => {
  const parts = value.split('/')
  return !/[\\\0:]/.test(value) && parts.length <= 16 && parts.every(part => part.length > 0 && part.length <= 128 && !part.startsWith('.'))
}, 'Unsafe relative resource path')
export const skillResourceSchema = z.object({ path: resourcePath, content }).strict()
export const agentProjectRequestSchema = z.discriminatedUnion('op', [
  z.object({ projectId: z.string(), op: z.literal('manifest') }).strict(),
  z.object({ projectId: z.string(), op: z.literal('read'), path: z.string() }).strict(),
  z.object({ projectId: z.string(), op: z.literal('writeSkill'), name, content, files: z.array(skillResourceSchema).max(63).default([]), ...write }).strict(),
  z.object({ projectId: z.string(), op: z.literal('writeMemory'), name, content: z.string().min(1).refine(value => new TextEncoder().encode(value).length <= 8192), confirmed: z.literal(true), ...write }).strict(),
  z.object({ projectId: z.string(), op: z.literal('writePolicy'), policy: projectPolicySchema, ...write }).strict(),
])
export const projectFileReferenceSchema = z.object({ path: z.string(), revision, bytes: z.number().int().nonnegative() })
/** Global resource hashes are lazy (null until read); project file hashes remain concrete. */
export const projectManifestFileReferenceSchema = projectFileReferenceSchema.extend({ revision: revision.nullable() })
export const projectArtifactSchema = z.object({
  kind: z.literal('artifact'), artifactId: z.string().uuid(), path: z.string(), revision,
  backupPath: z.string().nullable(), files: z.array(projectFileReferenceSchema), saved: z.literal(true), verified: z.literal(true),
})
export const projectResponseSchema = z.discriminatedUnion('kind', [
  projectArtifactSchema,
  z.object({ kind: z.literal('file'), path: z.string(), revision, contentRevision: revision, content: z.string() }),
  z.object({
    kind: z.literal('manifest'),
    skills: z.array(z.object({ name: z.string(), description: z.string(), path: z.string(), revision, source: z.enum(['project', 'global']), files: z.array(projectManifestFileReferenceSchema) })),
    mcp: z.array(z.object({ name: z.string(), enabled: z.boolean(), connectionType: z.enum(['stdio', 'http', 'unknown']), source: z.enum(['project', 'global']) })),
    memory: z.array(z.object({ path: z.string(), revision, summary: z.string(), confirmed: z.literal(true) })),
    policy: projectPolicySchema,
    provenance: z.object({ skills: z.string(), mcp: z.string(), policy: z.string().nullable(), resolution: z.literal('project-first'), fallback: z.string().nullable(), mcpAuthority: z.string() }),
    warnings: z.array(z.string()).default([]),
  }),
])
export type AgentProjectRequest = z.input<typeof agentProjectRequestSchema>
export type ProjectArtifact = z.infer<typeof projectArtifactSchema>
export type ProjectPolicy = z.infer<typeof projectPolicySchema>
export type ProjectBudgets = z.infer<typeof projectBudgetsSchema>
export function projectCapabilityAllowed(policy: ProjectPolicy, capability: z.infer<typeof capabilitySchema>, nativeAllowed: boolean): boolean {
  return nativeAllowed && !policy.deny.includes(capability) && (policy.allow.length === 0 || policy.allow.includes(capability))
}
export function restrictProjectBudgets(policy: ProjectBudgets, native: ProjectBudgets): ProjectBudgets {
  const minimum = (a: number | null | undefined, b: number | null | undefined) => a == null ? b : b == null ? a : Math.min(a, b)
  return { maxTurns: minimum(policy.maxTurns, native.maxTurns), repairLimit: minimum(policy.repairLimit, native.repairLimit), timeoutMs: minimum(policy.timeoutMs, native.timeoutMs) }
}
/** Pass manifest Skill/file paths unchanged to read (including global-skill:// references).
 * Global references are read-only, manifest-checked, and rejected when a project Skill overrides them.
 * Project SKILL.md revisions cover the package; global SKILL.md revisions hash the instruction text.
 * Global resource descriptor hashes are null until read; read responses always include contentRevision.
 */
export async function agentProjectCommand(request: AgentProjectRequest) {
  return projectResponseSchema.parse(await invoke('agent_project_command', { request: agentProjectRequestSchema.parse(request) }))
}
