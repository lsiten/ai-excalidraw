import {
  MousePointer2,
  PenLine,
  Highlighter,
  Eraser,
  Undo2,
  Redo2,
  ChevronLeft,
  ChevronRight,
  LayoutGrid,
  X,
  Trash2,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import type { AnnotationTool, InkHistory } from '@/lib/presentation/annotations'
export const INK_COLORS = ['#e03131', '#1971c2', '#2f9e44', '#f2b01e'] as const
const colors = ['红色', '蓝色', '绿色', '黄色'] as const
const tools = [
  { id: 'pointer', label: '指针 P', icon: MousePointer2 },
  { id: 'pen', label: '画笔 B', icon: PenLine },
  { id: 'highlighter', label: '荧光笔 H', icon: Highlighter },
  { id: 'eraser', label: '橡皮擦 E', icon: Eraser },
] as const
export function PlayerToolbar(props: {
  readonly tool: AnnotationTool
  readonly setTool: (tool: AnnotationTool) => void
  readonly color: string
  readonly setColor: (color: string) => void
  readonly index: number
  readonly count: number
  readonly history: InkHistory
  readonly go: (index: number) => void
  readonly undo: () => void
  readonly redo: () => void
  readonly clear: () => void
  readonly clearAll: () => void
  readonly exit: () => void
  readonly thumbnails: boolean
  readonly toggleThumbnails: () => void
}) {
  return (
    <div
      className="flex flex-wrap items-center justify-center gap-1 rounded-xl border border-border bg-card p-2 text-foreground shadow-xl"
      role="toolbar"
      aria-label="演示与临时批注工具"
    >
      <Button
        variant="ghost"
        size="icon"
        aria-label="页面缩略图"
        title="页面缩略图"
        aria-pressed={props.thumbnails}
        onClick={props.toggleThumbnails}
      >
        <LayoutGrid size={16} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="上一页"
        title="上一页 ←"
        disabled={props.index === 0}
        onClick={() => props.go(props.index - 1)}
      >
        <ChevronLeft size={18} />
      </Button>
      <span className="min-w-12 text-center text-xs tabular-nums" aria-live="polite">
        {props.index + 1} / {props.count}
      </span>
      <Button
        variant="ghost"
        size="icon"
        aria-label="下一页"
        title="下一页 → / 空格"
        disabled={props.index === props.count - 1}
        onClick={() => props.go(props.index + 1)}
      >
        <ChevronRight size={18} />
      </Button>
      <span className="mx-1 h-5 border-l border-border" />
      {tools.map((item) => (
        <Button
          key={item.id}
          variant="ghost"
          size="icon"
          title={item.label}
          aria-label={item.label}
          aria-pressed={props.tool === item.id}
          className={props.tool === item.id ? 'bg-primary/10 text-primary' : ''}
          onClick={() => props.setTool(item.id)}
        >
          <item.icon size={16} />
        </Button>
      ))}
      <div className="mx-1 flex items-center gap-1" role="group" aria-label="批注颜色">
        {INK_COLORS.map((color, index) => (
          <button
            key={color}
            aria-label={colors[index]}
            title={colors[index]}
            aria-pressed={props.color === color}
            onClick={() => props.setColor(color)}
            className="flex h-7 w-7 items-center justify-center rounded-full"
            style={{ backgroundColor: color }}
          >
            {props.color === color && (
              <span className="text-xs font-bold text-white" aria-hidden>
                ✓
              </span>
            )}
          </button>
        ))}
      </div>
      <Button
        variant="ghost"
        size="icon"
        aria-label="撤销批注"
        title="撤销批注 ⌘Z"
        disabled={!props.history.past.length}
        onClick={props.undo}
      >
        <Undo2 size={16} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="重做批注"
        title="重做批注 ⌘⇧Z"
        disabled={!props.history.future.length}
        onClick={props.redo}
      >
        <Redo2 size={16} />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="清空本页批注"
        title="清空本页批注"
        disabled={!props.history.present.length}
        onClick={props.clear}
      >
        <Eraser size={16} className="text-destructive" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="清空全部批注"
        title="清空全部批注"
        onClick={props.clearAll}
      >
        <Trash2 size={16} />
      </Button>
      <span className="mx-1 h-5 border-l border-border" />
      <Button variant="ghost" size="sm" className="gap-1" onClick={props.exit} title="退出演示 Esc">
        <X size={16} />
        退出
      </Button>
    </div>
  )
}
