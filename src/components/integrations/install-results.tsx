import { integrationNames, type IntegrationResult } from '@/lib/integrations'
import { cn } from '@/lib/utils'

export function IntegrationInstallResults({
  results,
}: {
  readonly results: readonly IntegrationResult[]
}) {
  return (
    <section aria-label="安装结果" aria-live="polite" className="space-y-3">
      <h3 className="text-sm font-medium">安装结果</h3>
      <ul className="space-y-3">
        {results.map((result) => (
          <li key={result.id} className="space-y-1 text-xs">
            <p className={cn('font-medium', !result.success && 'text-destructive')}>
              {integrationNames[result.id]} · {result.success ? '已完成' : '未完成'}
            </p>
            <p className="break-words">
              {result.message ||
                (result.success
                  ? '接入配置已就绪，无需重复修改。'
                  : '安装未完成，请刷新状态后重试。')}
            </p>
            {result.backupPaths.length > 0 && (
              <details className="text-muted-foreground">
                <summary className="w-fit cursor-pointer rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  查看备份路径（{result.backupPaths.length}）
                </summary>
                <ul className="mt-1 space-y-1">
                  {result.backupPaths.map((path) => (
                    <li key={path} className="select-text break-all font-mono">
                      {path}
                    </li>
                  ))}
                </ul>
              </details>
            )}
          </li>
        ))}
      </ul>
      <p className="text-xs text-muted-foreground">
        成功安装的接入，在对应开发工具重新加载 MCP / 重开会话后生效。
      </p>
    </section>
  )
}
