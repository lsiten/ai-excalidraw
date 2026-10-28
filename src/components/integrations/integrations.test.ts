import { beforeEach, describe, expect, it, vi } from 'vitest'
import { claimIntegrationPrompt, getIntegrationGuide, getIntegrationStatus, installIntegrations } from '@/lib/integrations'

const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))

beforeEach(() => { invoke.mockReset() })

describe('native integration boundary', () => {
  it('passes the current canvas project to status, installation and guide commands', async () => {
    // Given
    const status = { version: '0.4.0', serviceRunning: true, tools: [] }
    invoke.mockImplementation(async (command: string) => command === 'integration_status' ? status : command === 'integration_guide' ? { text: 'project guide' } : [{ id: 'pi', success: true, message: 'done', backupPaths: [] }])
    // When
    await getIntegrationStatus('project-A')
    await installIntegrations(['pi'], 'project-A')
    await getIntegrationGuide('project-A')
    // Then
    expect(invoke).toHaveBeenCalledWith('integration_status', { projectId: 'project-A' })
    expect(invoke).toHaveBeenCalledWith('integration_install', { tools: ['pi'], projectId: 'project-A' })
    expect(invoke).toHaveBeenCalledWith('integration_guide', { projectId: 'project-A' })
  })
  it('reads the complete native guide without invoking installation', async () => {
    const text = '# 安装指南\n\n```json\n{"path":"/本机/skill"}\n```\n'
    invoke.mockResolvedValue({ text })
    expect(await getIntegrationGuide()).toBe(text)
    expect(invoke).toHaveBeenCalledExactlyOnceWith('integration_guide')
  })

  it.each([{ text: '' }, { text: '  \n' }, { text: 42 }, {}])('rejects an unusable guide %j', async (response) => {
    invoke.mockResolvedValue(response)
    await expect(getIntegrationGuide()).rejects.toThrow()
  })

  it('never invokes installation for an empty selection', async () => {
    expect(await installIntegrations([])).toEqual([])
    expect(invoke).not.toHaveBeenCalled()
  })

  it('sends only selected tools and preserves independent failure, no-op and backup results', async () => {
    const results = [
      { id: 'codex', success: true, message: '已是最新，无需修改', backupPaths: [] },
      { id: 'cursor', success: false, message: '配置不可写', backupPaths: [] },
      { id: 'pi', success: true, message: '更新完成', backupPaths: ['/tmp/pi.bak'] },
    ]
    invoke.mockResolvedValue(results)
    expect(await installIntegrations(['codex', 'cursor', 'pi', 'codex'])).toEqual(results)
    expect(invoke).toHaveBeenCalledExactlyOnceWith('integration_install', { tools: ['codex', 'cursor', 'pi'] })
  })

  it('reports a missing per-tool result without dropping another tool success', async () => {
    const success = { id: 'codex', success: true, message: '已安装', backupPaths: ['/tmp/config.bak'] }
    invoke.mockResolvedValue([success])
    const results = await installIntegrations(['codex', 'claude'])
    expect(results).toHaveLength(2)
    expect(results[0]).toEqual(success)
    expect(results[1]).toMatchObject({ id: 'claude', success: false, backupPaths: [] })
  })

  it('accepts running service with no detected tools and keeps component states independent', async () => {
    const status = {
      version: '0.4.0', serviceRunning: true,
      tools: [{ id: 'codex', name: 'Codex', detected: false, configPath: '/tmp/config.toml',
        skillPath: '/tmp/skills', mcpStatus: 'current', skillStatus: 'missing', note: null }],
    }
    invoke.mockResolvedValue(status)
    expect(await getIntegrationStatus()).toEqual(status)
    expect(invoke).toHaveBeenCalledExactlyOnceWith('integration_status')
  })

  it('rejects malformed native status instead of rendering an unknown state', async () => {
    invoke.mockResolvedValue({ version: '0.4.0', serviceRunning: 'yes', tools: [] })
    await expect(getIntegrationStatus()).rejects.toThrow()
  })

  it.each([true, false])('uses the native prompt claim decision %s without installing anything', async (eligible) => {
    invoke.mockResolvedValue(eligible)
    expect(await claimIntegrationPrompt()).toBe(eligible)
    expect(invoke).toHaveBeenCalledExactlyOnceWith('integration_claim_prompt')
  })

  it('propagates native installation rejection to the UI error boundary', async () => {
    invoke.mockRejectedValue('文件被占用')
    await expect(installIntegrations(['codex'])).rejects.toBe('文件被占用')
  })
})
