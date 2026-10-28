import { SettingsDialog } from '@/components/settings-dialog'
import { PresentationActions } from '@/components/presentation/presentation-actions'
import { FolderOpen, Save, Download, PanelLeft, Check, AlertCircle } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { workspace } from '@/lib/workspace/store'
import { native } from '@/lib/workspace/documents'
import type { CanvasFile, Project } from '@/lib/workspace/types'

interface Props {
  readonly active: CanvasFile | undefined
  readonly project: Project | undefined
  readonly explorer: boolean
  readonly setExplorer: (visible: boolean) => void
  readonly disabled: boolean
  readonly pickFile: () => void
  readonly copy: (conflict?: boolean) => void
  readonly run: (action: () => Promise<unknown>) => void
}

export function WorkspaceHeader({
  active,
  project,
  explorer,
  setExplorer,
  disabled,
  pickFile,
  copy,
  run,
}: Props) {
  return (
    <header className="h-14 shrink-0 flex items-center gap-3 px-4 border-b border-border">
      {!explorer && (
        <Button
          variant="ghost"
          size="icon"
          title="显示文件栏"
          aria-label="显示文件栏"
          onClick={() => setExplorer(true)}
        >
          <PanelLeft size={16} />
        </Button>
      )}
      <div className="min-w-0 flex-1">
        <p className="text-sm truncate font-medium">
          {project?.name ?? '本地画布工作区'}
          {active && <span className="font-normal text-muted-foreground"> / {active.path}</span>}
        </p>
        {active && (
          <p className="text-[11px] text-muted-foreground flex items-center gap-1 mt-0.5">
            {active.status === 'saved' ? (
              <Check size={11} />
            ) : active.status === 'conflict' || active.status === 'error' ? (
              <AlertCircle size={11} />
            ) : (
              <Save size={11} />
            )}
            {
              {
                saved: '已保存到本地文件',
                dirty: '待保存',
                saving: '保存中…',
                conflict: '文件冲突',
                error: '保存失败',
              }[active.status]
            }
          </p>
        )}
      </div>
      <SettingsDialog projectId={active?.projectId ?? project?.id} projectDirectory={project?.directory} />
      <PresentationActions active={active} projectId={project?.id} disabled={disabled} />
      <Button variant="ghost" size="sm" onClick={pickFile} disabled={disabled} title="打开画布 ⌘O">
        <FolderOpen size={15} />
        <span className="ml-1">打开</span>
      </Button>
      {active && (
        <>
          <Button variant="ghost" size="sm" onClick={() => copy()} disabled={disabled}>
            另存为
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => run(() => native('export_canvas', { content: active.content }))}
            disabled={disabled}
          >
            <Download size={14} />
            <span className="ml-1">导出</span>
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled}
            onClick={() => run(() => workspace.flushAll())}
          >
            <Save size={14} />
            <span className="ml-1">保存</span>
          </Button>
        </>
      )}
    </header>
  )
}
