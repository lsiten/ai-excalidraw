import { usePageDrag } from './use-page-drag'
import { ArrowUp, ArrowDown, GripVertical, Crop } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { movePage, orderedPages, type Deck } from '@/lib/presentation/types'
import { SlidePreview } from './slide-preview'

export function DeckPages({
  deck,
  disabled,
  onChange,
}: {
  readonly deck: Deck
  readonly disabled: boolean
  readonly onChange: (deck: Deck) => void
}) {
  const order = orderedPages(deck)
  const move = (from: string, to: string) =>
    onChange({ ...deck, settings: { ...deck.settings, order: movePage(order, from, to) } })
  const { list, dragged, target, start, update, finish, click } = usePageDrag({
    disabled,
    move,
    select: (path) => onChange({ ...deck, selected: path }),
  })
  return (
    <nav
      ref={list}
      className="min-h-0 w-[240px] shrink-0 overflow-y-auto border-r border-border bg-secondary/10 p-3 max-[900px]:w-[200px]"
      aria-label="演示页顺序"
    >
      <p className="sticky top-0 z-10 mb-3 flex items-center gap-2 rounded-md bg-card px-2 py-2 text-xs font-medium text-foreground shadow-sm">
        <GripVertical size={16} />
        拖动卡片调整顺序
      </p>
      {order.map((path, index) => (
        <div
          key={path}
          data-slide-path={path}
          onPointerDown={(event) => start(event, path)}
          onPointerMove={update}
          onPointerUp={(event) => finish(event, true)}
          onPointerCancel={(event) => finish(event, false)}
          onLostPointerCapture={(event) => finish(event, false)}
          onClickCapture={(event) => click(event, path)}
          title="按住卡片或拖动手柄调整顺序"
          className={`relative mb-3 rounded-lg border border-border p-2 ${disabled ? '' : 'cursor-grab select-none touch-none active:cursor-grabbing'} ${dragged === path ? 'opacity-40' : ''} ${deck.selected === path ? 'bg-primary/10' : 'bg-card'}`}
        >
          <button
            type="button"
            className="block w-full cursor-grab rounded text-left active:cursor-grabbing"
            aria-label={`选择第 ${index + 1} 页 ${path}`}
            aria-current={deck.selected === path ? 'true' : undefined}
            disabled={disabled}
            onClick={() => onChange({ ...deck, selected: path })}
          >
            <SlidePreview
              projectId={deck.projectId}
              path={path}
              area={deck.data.areas[path]}
              className="aspect-video rounded bg-muted"
            />
            <span className="mt-2 flex items-center gap-1.5 text-xs">
              <span className="text-muted-foreground">{index + 1}</span>
              <span className="truncate" title={path}>
                {path
                  .split('/')
                  .at(-1)
                  ?.replace(/\.excalidraw$/, '')}
              </span>
              {deck.data.areas[path] && (
                <Crop size={12} className="ml-auto shrink-0 text-primary" />
              )}
            </span>
          </button>
          <div className="mt-1 flex items-center gap-1">
            <span className="mr-auto flex cursor-grab items-center gap-1 rounded bg-muted px-1.5 py-1 text-[11px] text-muted-foreground active:cursor-grabbing">
              <GripVertical size={16} aria-hidden />
              拖动排序
            </span>
            <Button
              data-page-arrow
              variant="ghost"
              size="icon"
              className="h-6 w-6"
              disabled={disabled || index === 0}
              aria-label={`将第 ${index + 1} 页上移`}
              onClick={() => {
                const to = order[index - 1]
                if (to) move(path, to)
              }}
            >
              <ArrowUp size={12} />
            </Button>
            <Button
              data-page-arrow
              variant="ghost"
              size="icon"
              className="h-6 w-6"
              disabled={disabled || index === order.length - 1}
              aria-label={`将第 ${index + 1} 页下移`}
              onClick={() => {
                const next = order[index + 1]
                if (next) move(next, path)
              }}
            >
              <ArrowDown size={12} />
            </Button>
          </div>
          {dragged && dragged !== path && target === path && (
            <div className="pointer-events-none absolute inset-0 flex items-center justify-center rounded-lg bg-primary/10">
              <span className="rounded-md bg-card px-3 py-2 text-xs font-medium text-primary shadow-lg">
                松开，移到第 {index + 1} 页
              </span>
            </div>
          )}
        </div>
      ))}
      {!order.length && (
        <p className="p-3 text-xs leading-6 text-muted-foreground">
          此文件夹没有画布。可以包含子文件夹，或创建 .excalidraw 文件后再播放。
        </p>
      )}
    </nav>
  )
}
