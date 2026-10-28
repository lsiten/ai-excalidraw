import { useEffect, useRef, useState } from 'react'
import { Loader2, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'
import {
  getIntegrationStatus,
  installIntegrations,
  integrationErrorMessage,
} from '@/lib/integrations'
import type { IntegrationId, IntegrationResult, IntegrationStatus } from '@/lib/integrations'
import { IntegrationToolRow } from './tool-row'
import { IntegrationInstallResults } from './install-results'
import { IntegrationGuideDialog } from './integration-guide-dialog'

export function IntegrationsPanel({
  onInstallingChange,
  projectId,
  projectDirectory,
}: {
  readonly projectId?: string
  readonly projectDirectory?: string
  readonly onInstallingChange: (busy: boolean) => void
}) {
  const [status, setStatus] = useState<IntegrationStatus | null>(null)
  const [selected, setSelected] = useState<readonly IntegrationId[]>([])
  const [refreshing, setRefreshing] = useState(true)
  const [installing, setInstalling] = useState(false)
  const [statusError, setStatusError] = useState('')
  const [installError, setInstallError] = useState('')
  const [results, setResults] = useState<readonly IntegrationResult[]>([])
  const operation = useRef(false)

  useEffect(() => {
    let active = true
    getIntegrationStatus(projectId)
      .then(
        (value) => {
          if (active) setStatus(value)
        },
        (error: unknown) => {
          if (active) setStatusError(integrationErrorMessage(error))
        },
      )
      .finally(() => {
        if (active) setRefreshing(false)
      })
    return () => {
      active = false
    }
  }, [projectId])

  async function refreshStatus() {
    setRefreshing(true)
    setStatusError('')
    try {
      setStatus(await getIntegrationStatus(projectId))
    } catch (error) {
      setStatusError(integrationErrorMessage(error))
    } finally {
      setRefreshing(false)
    }
  }

  async function installSelected() {
    if (operation.current || refreshing || selected.length === 0) return
    operation.current = true
    setInstalling(true)
    onInstallingChange(true)
    setInstallError('')
    setResults([])
    try {
      setResults(await installIntegrations(selected, projectId))
    } catch (error) {
      setInstallError(integrationErrorMessage(error))
    } finally {
      await refreshStatus()
      operation.current = false
      setInstalling(false)
      onInstallingChange(false)
    }
  }

  const busy = refreshing || installing
  const detected = status?.tools.filter((tool) => tool.detected).map((tool) => tool.name) ?? []

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto overscroll-contain px-6 py-4">
        <div className="flex items-start justify-between gap-3">
          <div className="space-y-1">
            <h3 className="text-sm font-medium">选择要接入的开发工具</h3>
            <p className="text-xs text-muted-foreground">
              让开发工具通过 MCP 操作画布，并安装配套 Skill。无需填写 API Key。
            </p>
          </div>
          <Button
            size="sm"
            variant="ghost"
            disabled={busy}
            onClick={() => void refreshStatus()}
            className="shrink-0 gap-2"
          >
            {refreshing ? (
              <Loader2
                aria-hidden="true"
                className="h-4 w-4 animate-spin motion-reduce:animate-none"
              />
            ) : (
              <RefreshCw aria-hidden="true" className="h-4 w-4" />
            )}
            刷新
          </Button>
        </div>
        <p className="break-words rounded-lg bg-muted p-3 text-xs text-muted-foreground">
          {projectId ? <>优先安装到当前画布所属项目的 <span className="font-mono">.agents</span> 目录，项目内工具共用这份 MCP 配置与 Skill。<br /><span className="select-text">{projectDirectory ? `${projectDirectory}/.agents` : '.agents'}</span></> : '当前未选择项目，将使用各工具的用户配置目录。打开项目后可安装到项目的 .agents 目录。'}
        </p>
        <div role="status" className="space-y-1 text-xs text-muted-foreground">
          {refreshing && <p>正在读取接入状态…</p>}
          {status && (
            <>
              <p>已检测到：{detected.length ? detected.join('、') : '暂无工具，可提前配置'}</p>
              <p>
                应用 {status.version} · Skill 1.5.0 · MCP{' '}
                {status.serviceRunning ? '运行中' : '未运行'}
              </p>
            </>
          )}
        </div>
        {statusError && (
          <p role="alert" className="break-words text-xs text-destructive">
            状态读取失败：{statusError}
            {status ? ' 当前显示上次状态，请刷新后确认。' : ''}
          </p>
        )}
        {status && (
          <ul aria-label="开发工具" className="space-y-2">
            {status.tools.map((tool) => (
              <IntegrationToolRow
                key={tool.id}
                tool={tool}
                selected={selected.includes(tool.id)}
                busy={busy}
                onSelect={(checked) =>
                  setSelected((previous) =>
                    checked ? [...previous, tool.id] : previous.filter((id) => id !== tool.id),
                  )
                }
              />
            ))}
          </ul>
        )}
        {status?.tools.length === 0 && (
          <p className="text-sm text-muted-foreground">暂无可配置的工具，请刷新重试。</p>
        )}
        <div className="space-y-2 rounded-lg bg-muted p-3 text-xs text-muted-foreground">
          <p>
            仅安装 /
            更新所选工具；已有配置修改前会备份，并保留其他配置。未检测到的工具也可提前配置。
          </p>
          <p>
            桌面端 MCP
            随应用独立启动和关闭，无需开发工具连接。连接器不会启动桌面应用；使用接入时请保持应用打开。
          </p>
        </div>
        {installError && (
          <p role="alert" className="break-words text-xs text-destructive">
            安装未完成：{installError}
          </p>
        )}
        {results.length > 0 && <IntegrationInstallResults results={results} />}
      </div>
      <div className="flex shrink-0 flex-col gap-3 border-t border-border bg-card px-6 py-4 sm:flex-row sm:items-center sm:justify-between">
        <div role="status" className="space-y-1 text-xs text-muted-foreground">
          <p>
            {installing
              ? '正在安装 / 更新，请稍候…'
              : selected.length
                ? `已选择 ${selected.length} 个工具`
                : '请先选择开发工具'}
          </p>
          {!installing && results.length > 0 && (
            <p>
              上次安装：{results.filter((result) => result.success).length} 项完成，
              {results.filter((result) => !result.success).length} 项未完成
            </p>
          )}
          {!installing && installError && (
            <p className="text-destructive">安装未完成，请查看详情</p>
          )}
        </div>
        <div className="flex shrink-0 items-center justify-end gap-2">
          <IntegrationGuideDialog projectId={projectId} />
          <Button
            disabled={busy || selected.length === 0}
            onClick={() => void installSelected()}
            className="shrink-0 gap-2"
          >
            {installing && (
              <Loader2
                aria-hidden="true"
                className="h-4 w-4 animate-spin motion-reduce:animate-none"
              />
            )}
            安装 / 更新
          </Button>
        </div>
      </div>
    </div>
  )
}
