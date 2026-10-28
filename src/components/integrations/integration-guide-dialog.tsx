import { useEffect, useId, useRef, useState } from 'react'
import * as Dialog from '@radix-ui/react-dialog'
import { Loader2, X } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { getIntegrationGuide, integrationErrorMessage } from '@/lib/integrations'
import { copyIntegrationGuide } from './guide-clipboard'

function GuideContents({ projectId }: { readonly projectId?: string }) {
  const [text, setText] = useState('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [copyState, setCopyState] = useState<'idle' | 'copying' | 'copied' | 'manual'>('idle')
  const textarea = useRef<HTMLTextAreaElement>(null)
  const id = useId()

  useEffect(() => {
    let active = true
    getIntegrationGuide(projectId)
      .then(
        (value) => {
          if (active) setText(value)
        },
        (reason: unknown) => {
          if (active) setError(integrationErrorMessage(reason))
        },
      )
      .finally(() => {
        if (active) setLoading(false)
      })
    return () => {
      active = false
    }
  }, [projectId])

  async function retry() {
    setLoading(true)
    setError('')
    try {
      setText(await getIntegrationGuide(projectId))
    } catch (reason) {
      setError(integrationErrorMessage(reason))
    } finally {
      setLoading(false)
    }
  }

  async function copy() {
    if (!textarea.current || copyState === 'copying') return
    setCopyState('copying')
    setCopyState(await copyIntegrationGuide(textarea.current))
  }

  return (
    <>
      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        {loading && (
          <p role="status" className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2
              aria-hidden="true"
              className="h-4 w-4 animate-spin motion-reduce:animate-none"
            />
            正在读取安装指南…
          </p>
        )}
        {error && (
          <div className="space-y-2">
            <p role="alert" className="break-words text-sm text-destructive">
              读取安装指南失败：{error}
            </p>
            <Button variant="outline" onClick={() => void retry()}>
              重试
            </Button>
          </div>
        )}
        {text && (
          <>
            <label htmlFor={id} className="mb-2 block text-sm font-medium">
              复制以下完整内容给你的 AI 工具
            </label>
            <textarea
              ref={textarea}
              id={id}
              value={text}
              readOnly
              spellCheck={false}
              aria-describedby={`${id}-copy-status`}
              className="h-[40dvh] min-h-32 w-full resize-none select-text whitespace-pre-wrap break-words rounded-lg border border-input bg-background p-3 font-mono text-xs leading-relaxed text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
          </>
        )}
      </div>
      <div className="shrink-0 space-y-3 border-t border-border px-6 py-4">
        <p id={`${id}-copy-status`} role="status" className="text-xs text-muted-foreground">
          {copyState === 'manual'
            ? '自动复制失败，文本已选中。请按 ⌘C / Ctrl+C，或使用右键菜单复制。'
            : copyState === 'copied'
              ? '已复制，可粘贴给 AI 工具。'
              : copyState === 'copying'
                ? '正在复制…'
                : '指南仅供复制，不会自动修改开发工具配置。'}
        </p>
        <div className="flex flex-wrap justify-end gap-2">
          {copyState === 'manual' && (
            <Button
              variant="outline"
              onClick={() => {
                textarea.current?.focus()
                textarea.current?.select()
              }}
            >
              选择全部文本
            </Button>
          )}
          <Dialog.Close asChild>
            <Button variant="outline">关闭</Button>
          </Dialog.Close>
          <Button
            disabled={!text || loading || copyState === 'copying'}
            onClick={() => void copy()}
          >
            一键复制给 AI
          </Button>
        </div>
      </div>
    </>
  )
}

export function IntegrationGuideDialog({ projectId }: { readonly projectId?: string }) {
  return (
    <Dialog.Root>
      <Dialog.Trigger asChild>
        <Button variant="outline" className="shrink-0">
          安装指南
        </Button>
      </Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[60] bg-black/50 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-1/2 z-[61] flex max-h-[90dvh] w-[calc(100%_-_2rem)] max-w-[640px] -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-xl border border-border bg-card shadow-xl">
          <div className="shrink-0 px-6 pt-6">
            <div className="flex items-center justify-between gap-3">
              <Dialog.Title className="text-lg font-semibold">安装指南</Dialog.Title>
              <Dialog.Close asChild>
                <Button variant="ghost" size="icon" aria-label="关闭安装指南">
                  <X aria-hidden="true" className="h-4 w-4" />
                </Button>
              </Dialog.Close>
            </div>
            <Dialog.Description className="mt-2 text-sm text-muted-foreground">
              工具不在列表中？将指南交给你的 AI 工具，了解 MCP 连接配置与 Skill 安装方式。
            </Dialog.Description>
          </div>
          <GuideContents projectId={projectId} />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  )
}
