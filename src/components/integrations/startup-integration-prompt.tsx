import { useEffect, useState } from 'react'
import { isTauri } from '@tauri-apps/api/core'
import * as Dialog from '@radix-ui/react-dialog'
import { Button } from '@/components/ui/button'
import { claimIntegrationPrompt, integrationErrorMessage } from '@/lib/integrations'
import { IntegrationDialog } from './integration-dialog'

// Share the native claim across StrictMode effect replay; only a live mount consumes it.
let startupClaim: Promise<boolean> | undefined
let presented = false

export function StartupIntegrationPrompt({ projectId, projectDirectory }: { readonly projectId?: string; readonly projectDirectory?: string }) {
  const [promptOpen, setPromptOpen] = useState(false)
  const [selectorOpen, setSelectorOpen] = useState(false)

  useEffect(() => {
    if (!isTauri() || presented) return
    let active = true
    startupClaim ??= claimIntegrationPrompt()
    startupClaim
      .then((eligible) => {
        if (active && eligible && !presented) {
          presented = true
          setPromptOpen(true)
        }
      })
      .catch((error: unknown) => {
        if (active) console.error('无法检查首次接入提示：', integrationErrorMessage(error))
      })
    return () => {
      active = false
    }
  }, [])

  return (
    <>
      <Dialog.Root open={promptOpen} onOpenChange={setPromptOpen}>
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm" />
          <Dialog.Content
            className="fixed left-1/2 top-1/2 z-50 w-[calc(100%_-_2rem)] max-w-md -translate-x-1/2 -translate-y-1/2 rounded-xl border border-border bg-card p-6 shadow-xl"
            onCloseAutoFocus={(event) => {
              if (selectorOpen) event.preventDefault()
            }}
          >
            <Dialog.Title className="text-lg font-semibold">连接你的 AI 开发工具</Dialog.Title>
            <Dialog.Description className="mt-3 text-sm leading-relaxed text-muted-foreground">
              桌面端 MCP 已随应用启动。安装 Skill 与连接配置后，可以让本机 AI 工具操作画布。
            </Dialog.Description>
            <div className="mt-6 flex justify-end gap-2">
              <Dialog.Close asChild>
                <Button variant="outline">以后再说</Button>
              </Dialog.Close>
              <Button
                onClick={() => {
                  setPromptOpen(false)
                  setSelectorOpen(true)
                }}
              >
                选择工具
              </Button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      <IntegrationDialog projectId={projectId} projectDirectory={projectDirectory} open={selectorOpen} onOpenChange={setSelectorOpen} />
    </>
  )
}
