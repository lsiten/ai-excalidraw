import * as Dialog from '@radix-ui/react-dialog'
import { Button } from '@/components/ui/button'
import { useLocale } from '@/hooks/use-locale'

interface ConfirmDialogProps {
  open: boolean
  title: string
  description?: string
  confirmLabel?: string
  cancelLabel?: string
  destructive?: boolean
  zIndex?: number
  onConfirm: () => void
  onOpenChange: (open: boolean) => void
}

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  cancelLabel,
  destructive = false,
  zIndex = 50,
  onConfirm,
  onOpenChange,
}: ConfirmDialogProps) {
  const { t } = useLocale()

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay
          style={{ zIndex }}
          className="fixed inset-0 bg-black/50 backdrop-blur-sm z-50 animate-in fade-in"
        />
        <Dialog.Content
          style={{ zIndex }}
          className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-50 w-full max-w-sm rounded-xl bg-card border border-border shadow-xl p-6 animate-in fade-in"
        >
          <Dialog.Title className="text-lg font-semibold mb-2">{title}</Dialog.Title>
          {description && (
            <Dialog.Description className="text-sm text-muted-foreground mb-6">
              {description}
            </Dialog.Description>
          )}
          <div className="flex justify-end gap-2">
            <Dialog.Close asChild>
              <Button variant="outline">{cancelLabel ?? t.cancel}</Button>
            </Dialog.Close>
            <Button
              variant={destructive ? 'destructive' : 'default'}
              onClick={() => {
                onConfirm()
                onOpenChange(false)
              }}
            >
              {confirmLabel ?? t.confirm}
            </Button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
