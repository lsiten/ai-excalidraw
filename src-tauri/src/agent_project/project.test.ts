import { describe, expect, it, vi } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { agentProjectCommand, agentProjectRequestSchema, projectArtifactSchema, projectCapabilityAllowed, projectPolicySchema, projectResponseSchema, restrictProjectBudgets } from '../../../src/lib/agent/project'
const mock = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mock.invoke }))

describe('project service contract', () => {
  const fixture = JSON.parse(readFileSync(resolve(process.env.AGENT_PROJECT_CONTRACT_FIXTURE ?? 'src-tauri/src/agent_project/fixtures/command.json'), 'utf8'))
  it('parses actual Rust dispatch request, artifact, preview and manifest', () => {
    expect(agentProjectRequestSchema.parse(fixture.request).op).toBe('writeSkill')
    for (const key of ['artifact', 'preview', 'manifest']) expect(projectResponseSchema.safeParse(fixture[key]).success).toBe(true)
    expect(fixture.artifact.revision).toBe(fixture.preview.revision)
  })
  it('invokes the registered command and retains resource payload and artifact references', async () => {
    mock.invoke.mockResolvedValueOnce(fixture.artifact)
    const artifact = await agentProjectCommand(fixture.request)
    expect(mock.invoke).toHaveBeenCalledWith('agent_project_command', { request: agentProjectRequestSchema.parse(fixture.request) })
    expect(artifact).toEqual(fixture.artifact)
  })
  it('requires confirmation and bounds resource paths and bytes', () => {
    for (const path of ['../escape', '/absolute', 'a//b', 'a/./b', 'a\\b', '.hidden']) {
      expect(agentProjectRequestSchema.safeParse({ ...fixture.request, files: [{ path, content: 'x' }] }).success).toBe(false)
    }
    expect(agentProjectRequestSchema.safeParse({ ...fixture.request, files: [{ path: 'large.txt', content: '中'.repeat(90000) }] }).success).toBe(false)
    expect(agentProjectRequestSchema.safeParse({ projectId: 'p', op: 'writeMemory', name: 'fact', content: 'fact', authorized: true, confirmed: false, expectedRevision: null }).success).toBe(false)
    expect(agentProjectRequestSchema.safeParse({ ...fixture.request, authorized: false }).success).toBe(false)
  })
  it('intersects native permission and budgets with deny-wins project policy', () => {
    const policy = projectPolicySchema.parse({ allow: ['mcp'], deny: ['mcp'] })
    expect(projectCapabilityAllowed(policy, 'mcp', true)).toBe(false)
    expect(projectCapabilityAllowed(projectPolicySchema.parse({}), 'mcp', false)).toBe(false)
    expect(restrictProjectBudgets({ maxTurns: 100, repairLimit: 1 }, { maxTurns: 4, repairLimit: 3, timeoutMs: 500 })).toEqual({ maxTurns: 4, repairLimit: 1, timeoutMs: 500 })
  })
})

describe('global Skill read contract', () => {
  const fixture = JSON.parse(readFileSync(resolve(process.env.AGENT_GLOBAL_CONTRACT_FIXTURE ?? 'src-tauri/src/agent_project/fixtures/global-read.json'), 'utf8'))
  it('accepts native manifest global references and their verified instruction/resource responses', () => {
    expect(projectResponseSchema.parse(fixture.manifest).kind).toBe('manifest')
    for (const key of ['instructions', 'resource']) {
      const response = projectResponseSchema.parse(fixture[key])
      expect(response.kind).toBe('file')
      expect(fixture[key].path).toMatch(/^global-skill:\/\/demo\//)
    }
    expect(fixture.readPolicyEnforced).toBe(true)
    expect(fixture.globalReferenceRejectedAfterProjectOverride).toBe(true)
    expect(fixture.absoluteHomePathRejected).toBe(true)
  })
  it('allows lazy global resource hashes while read receipts retain concrete content hashes', () => {
    const entry = fixture.manifest.skills[0].files.find((file: { path: string }) => file.path === fixture.resource.path)
    expect(entry.revision).toBeNull()
    expect(fixture.resource.contentRevision).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(projectArtifactSchema.safeParse({ kind: 'artifact', artifactId: '00000000-0000-4000-8000-000000000000', path: 'x', revision: fixture.resource.contentRevision, backupPath: null, saved: true, verified: true, files: [{ path: 'x', bytes: 1, revision: null }] }).success).toBe(false)
  })
  it('passes a discovered global file reference unchanged through the same command without allowing global write names', async () => {
    const request = { projectId: 'project', op: 'read' as const, path: fixture.resource.path }
    mock.invoke.mockResolvedValueOnce(fixture.resource)
    expect(await agentProjectCommand(request)).toEqual(fixture.resource)
    expect(mock.invoke).toHaveBeenCalledWith('agent_project_command', { request })
    expect(agentProjectRequestSchema.safeParse({ projectId: 'project', op: 'writeSkill', name: 'global-skill://demo', content: 'text', expectedRevision: null, authorized: true }).success).toBe(false)
  })
})
