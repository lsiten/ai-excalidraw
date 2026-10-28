import { StartupIntegrationPrompt } from '@/components/integrations/startup-integration-prompt'
import { useEffect, useState } from 'react'
import { FolderOpen, X, AlertCircle } from 'lucide-react'
import { useWorkspace } from '@/hooks/use-workspace'
import { workspace } from '@/lib/workspace/store'
import { Button } from '@/components/ui/button'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { CanvasTab } from './canvas-tab'
import { Explorer } from './explorer'
import { ExplorerPane } from './explorer-pane'
import { FileDialog } from './file-dialog'
import { WorkspaceHeader } from './workspace-header'
import { WorkspaceTabs } from './workspace-tabs'
import { useWorkspaceActions } from './use-workspace-actions'
import { PresentationHost } from '@/components/presentation'
import { presentations } from '@/lib/presentation/controller'
import { migrateLegacy } from '@/lib/workspace/migration'

export function WorkspaceShell() {
  const state = useWorkspace()
  const [explorer, setExplorer] = useState(() => window.innerWidth >= 1000)
  const [narrow, setNarrow] = useState(() => window.innerWidth < 1000)
  useEffect(() => {
    const resize = () => {
      const isNarrow = window.innerWidth < 1000
      setNarrow(isNarrow)
      if (isNarrow) setExplorer(false)
    }
    window.addEventListener('resize', resize)
    return () => window.removeEventListener('resize', resize)
  }, [])
  const [reloadId, setReloadId] = useState<string | null>(null)
  const active = state.documents.find((doc) => doc.id === state.activeId)
  const project = state.projects.find((p) => p.id === (active?.projectId ?? state.projectId))
  const disabled = !state.initialized || state.busy || state.aiBusy
  const {
    dialog,
    setDialog,
    deleting,
    setDeleting,
    run,
    pickProject,
    pickFile,
    newCanvas,
    newDirectory,
    importFile,
    copy,
    action,
  } = useWorkspaceActions(state, active)
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (state.presenting || !(event.metaKey || event.ctrlKey) || state.busy || state.aiBusy)
        return
      if (event.key.toLowerCase() === 's') {
        event.preventDefault()
        run(() => workspace.flushAll())
      }
      if (event.key.toLowerCase() === 'o') {
        event.preventDefault()
        if (event.shiftKey) pickProject()
        else pickFile()
      }
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  }) // Always uses the latest project and tab.
  return (
    <>
      <div
        inert={!!state.presenting}
        className="relative h-full flex bg-background text-foreground"
      >
        {explorer && (
          <ExplorerPane narrow={narrow}>
            <Explorer
              state={state}
              openProject={pickProject}
              selectProject={(id) => run(() => workspace.selectProject(id))}
              openFile={(path) => {
                if (state.projectId) run(() => workspace.openCanvas(state.projectId ?? '', path))
              }}
              newCanvas={newCanvas}
              newDirectory={newDirectory}
              importFile={importFile}
              refresh={() => run(() => workspace.checkExternal(true))}
              action={action}
              collapse={() => setExplorer(false)}
              onPresent={(folder) => {
                if (state.projectId)
                  void presentations.prepare(state.projectId, folder).catch((error) =>
                    workspace.set({
                      error: error instanceof Error ? error.message : String(error),
                    }),
                  )
              }}
            />
          </ExplorerPane>
        )}
        <main className="flex-1 min-w-0 flex flex-col relative" aria-label="画布工作区">
          <WorkspaceHeader
            active={active}
            project={project}
            explorer={explorer}
            setExplorer={setExplorer}
            disabled={disabled}
            pickFile={pickFile}
            copy={copy}
            run={run}
          />
          {state.error && (
            <div
              role="alert"
              className="px-4 py-2 text-xs text-destructive bg-destructive/5 flex items-center gap-2"
            >
              <p className="flex-1 break-words">{state.error}</p>
              <button
                aria-label="关闭错误提示"
                className="p-1"
                onClick={() => workspace.set({ error: null })}
              >
                <X size={14} />
              </button>
            </div>
          )}
          <WorkspaceTabs state={state} disabled={disabled} run={run} />
          {active && (active.status === 'conflict' || active.status === 'error') && (
            <div
              role="alert"
              className="px-4 py-3 bg-secondary border-b border-border flex flex-wrap items-center gap-3 text-xs"
            >
              <AlertCircle size={16} />
              <p className="flex-1 min-w-48">{active.error}</p>
              <Button size="sm" variant="outline" disabled={disabled} onClick={() => copy(true)}>
                另存副本
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={disabled}
                onClick={() => setReloadId(active.id)}
              >
                重新加载文件
              </Button>
            </div>
          )}
          <div className="flex-1 min-h-0 relative">
            {state.documents.map((file) => (
              <div
                key={file.tabId}
                role="tabpanel"
                id={`canvas-${file.id}`}
                className="h-full"
                hidden={file.id !== state.activeId}
              >
                <CanvasTab
                  file={file}
                  active={file.id === state.activeId}
                  locked={!state.initialized || state.busy}
                />
              </div>
            ))}
            {!active && (
              <div className="h-full flex flex-col items-center justify-center text-center p-8">
                <div className="rounded-2xl bg-muted p-5 mb-6">
                  <FolderOpen size={32} className="text-muted-foreground" />
                </div>
                <h1 className="text-xl font-semibold">让每张画布都有自己的位置</h1>
                <p className="text-sm text-muted-foreground mt-3 max-w-md leading-7">
                  打开本地文件夹作为项目，在目录中创建、整理画布。所有图形都保存为可直接使用的
                  .excalidraw 文件。
                </p>
                <div className="flex gap-3 mt-6">
                  <Button onClick={pickProject} disabled={disabled || !state.initialized}>
                    打开本地项目
                  </Button>
                  {state.projectId && (
                    <Button variant="outline" onClick={newCanvas} disabled={disabled}>
                      新建画布
                    </Button>
                  )}
                </div>
                {state.projectId && (
                  <button
                    className="text-xs text-muted-foreground underline mt-6"
                    disabled={disabled}
                    onClick={() => run(() => migrateLegacy(state.projectId ?? ''))}
                  >
                    将旧版缓存画布迁移到当前项目
                  </button>
                )}
                <p className="text-xs text-muted-foreground mt-8">
                  ⌘O 打开画布 · ⌘⇧O 打开项目 · ⌘S 保存
                </p>
              </div>
            )}
          </div>
          {disabled && (
            <div className="absolute inset-x-0 bottom-0 z-30 text-center text-xs py-1.5 bg-primary text-primary-foreground">
              {!state.initialized
                ? state.loading
                : state.aiBusy
                  ? 'AI 正在操作当前文件，可在 AI 面板中停止'
                  : '正在处理项目文件…'}
            </div>
          )}
        </main>
        <ConfirmDialog
          open={!!reloadId}
          onOpenChange={(open) => {
            if (!open) setReloadId(null)
          }}
          title="重新加载文件？"
          description="当前画布未保存的修改将被磁盘版本替换。需要保留修改时，请先另存副本。"
          confirmLabel="重新加载"
          cancelLabel="取消"
          onConfirm={() => {
            if (reloadId) run(() => workspace.reload(reloadId))
          }}
        />
        {dialog && <FileDialog request={dialog} onClose={() => setDialog(null)} />}
        <ConfirmDialog
          open={!!deleting}
          onOpenChange={(open) => {
            if (!open) setDeleting(null)
          }}
          title={`将“${deleting?.name ?? ''}”移到废纸篓？`}
          description={
            deleting?.kind === 'directory'
              ? '目录及其中全部文件将一起移动到系统废纸篓。'
              : '原文件将移动到系统废纸篓，可在 Finder 中恢复。'
          }
          confirmLabel="移到废纸篓"
          cancelLabel="取消"
          destructive
          onConfirm={() => {
            if (deleting && state.projectId)
              run(() =>
                workspace.mutate('delete_file', {
                  projectId: state.projectId ?? '',
                  path: deleting.path,
                }),
              )
          }}
        />
      </div>
      <StartupIntegrationPrompt projectId={project?.id} projectDirectory={project?.directory} />
      <PresentationHost />
    </>
  )
}
