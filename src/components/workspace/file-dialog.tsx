import { useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { message } from '@/lib/workspace/types'
export interface FileDialogRequest {
  readonly title: string
  readonly description: string
  readonly initial: string
  readonly submit: (path: string) => Promise<unknown>
}
export function FileDialog({
  request,
  onClose,
}: {
  readonly request: FileDialogRequest
  readonly onClose: () => void
}) {
  const [value, setValue] = useState(request.initial)
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)
  return (
    <Dialog.Root
      open={!!request}
      onOpenChange={(open) => {
        if (!open && !saving) onClose()
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/40 z-[100]" />
        <Dialog.Content className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-[101] w-[440px] max-w-[90vw] rounded-xl bg-card border border-border shadow-xl p-6">
          <Dialog.Title className="text-lg font-semibold">{request?.title}</Dialog.Title>
          <Dialog.Description className="text-sm text-muted-foreground mt-2 mb-5">
            {request?.description}
          </Dialog.Description>
          <form
            onSubmit={(event) => {
              event.preventDefault()
              if (!request || saving) return
              setSaving(true)
              setError('')
              void request
                .submit(value.trim())
                .then(onClose)
                .catch((error) => setError(message(error)))
                .finally(() => setSaving(false))
            }}
          >
            <label className="block text-sm mb-2" htmlFor="workspace-file-name">
              项目内的相对路径
            </label>
            <Input
              id="workspace-file-name"
              value={value}
              onChange={(event) => setValue(event.target.value)}
              disabled={saving}
              autoFocus
            />
            <p className="text-xs text-muted-foreground mt-2">
              支持子目录，例如：支付模块/退款流程.excalidraw
            </p>
            {error && (
              <p role="alert" className="text-sm text-destructive mt-3 break-words">
                {error}
              </p>
            )}
            <div className="flex justify-end gap-2 mt-6">
              <Button type="button" variant="outline" onClick={onClose} disabled={saving}>
                取消
              </Button>
              <Button type="submit" disabled={saving || !value.trim()}>
                {saving ? '处理中…' : '确定'}
              </Button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
