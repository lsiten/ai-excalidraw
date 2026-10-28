import { useState } from 'react'
import { ConfirmDialog } from '@/components/ui/confirm-dialog'
import { FilePenLine, X } from 'lucide-react'
import { workspace } from '@/lib/workspace/store'
import type { WorkspaceSnapshot } from '@/lib/workspace/types'

interface Props {
  readonly state: WorkspaceSnapshot
  readonly disabled: boolean
  readonly run: (action: () => Promise<unknown>) => void
}

export function WorkspaceTabs({ state, disabled, run }: Props) {
  const [discardId, setDiscardId] = useState<string | null>(null)
  return (
    <>
      {state.documents.length > 0 && (
        <div
          role="tablist"
          aria-label="已打开的画布"
          className="flex shrink-0 overflow-x-auto border-b border-border bg-muted/30"
        >
          {state.documents.map((doc) => (
            <div
              key={doc.id}
              className={`flex shrink-0 items-center border-r border-border max-w-64 ${doc.id === state.activeId ? 'bg-background text-primary' : 'text-muted-foreground'}`}
            >
              <button
                role="tab"
                aria-selected={doc.id === state.activeId}
                aria-controls={`canvas-${doc.id}`}
                className="flex items-center gap-2 pl-3 pr-2 h-10 text-xs min-w-0"
                title={`${state.projects.find((p) => p.id === doc.projectId)?.directory}/${doc.path}`}
                disabled={disabled}
                onClick={() => run(() => workspace.activate(doc.id))}
              >
                <FilePenLine size={13} />
                <span className="truncate">
                  {doc.path
                    .split('/')
                    .at(-1)
                    ?.replace(/\.excalidraw$/, '')}
                </span>
                {doc.status !== 'saved' && <span aria-label="未保存">•</span>}
              </button>
              <button
                aria-label={`关闭 ${doc.path}`}
                className="p-2 mr-1 hover:bg-muted rounded"
                disabled={disabled}
                onClick={() => {
                  if (doc.status === 'conflict' || doc.status === 'error') setDiscardId(doc.id)
                  else run(() => workspace.close(doc.id))
                }}
              >
                <X size={12} />
              </button>
            </div>
          ))}
        </div>
      )}
      <ConfirmDialog
        open={!!discardId}
        onOpenChange={(open) => {
          if (!open) setDiscardId(null)
        }}
        title="关闭未保存的画布？"
        description="未保存的修改将被放弃，磁盘文件保持原样。需要保留内容时，请取消并先另存副本。"
        confirmLabel="放弃修改并关闭"
        cancelLabel="取消"
        destructive
        onConfirm={() => {
          if (discardId) run(() => workspace.close(discardId, true))
        }}
      />
    </>
  )
}
