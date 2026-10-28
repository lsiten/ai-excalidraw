import { useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { IntegrationsPanel } from './integrations-panel'

type IntegrationDialogProps = {
  readonly projectId?: string
  readonly projectDirectory?: string
  readonly open: boolean
  readonly onOpenChange: (open: boolean) => void
}

export function IntegrationDialog({ open, onOpenChange, projectId, projectDirectory }: IntegrationDialogProps) {
  const [installing, setInstalling] = useState(false)
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(next) => {
        if (!installing) onOpenChange(next)
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-50 flex max-h-[90dvh] w-[calc(100%_-_2rem)] max-w-[640px] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl">
          <div className="flex shrink-0 items-center justify-between px-6 pb-2 pt-6">
            <Dialog.Title className="text-lg font-semibold">安装 AI 工具</Dialog.Title>
            <Dialog.Close asChild>
              <Button variant="ghost" size="icon" aria-label="关闭工具安装" disabled={installing}>
                <X aria-hidden="true" className="h-4 w-4" />
              </Button>
            </Dialog.Close>
          </div>
          <Dialog.Description className="sr-only">
            选择需要安装或更新 MCP 与 Skill 的本机开发工具。
          </Dialog.Description>
          <IntegrationsPanel key={projectId ?? 'global'} projectId={projectId} projectDirectory={projectDirectory} onInstallingChange={setInstalling} />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
