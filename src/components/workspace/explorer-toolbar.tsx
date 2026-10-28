import { Plus, FolderPlus, Upload, RefreshCw } from 'lucide-react'
import { Button } from '@/components/ui/button'

interface Props {
  readonly disabled: boolean
  readonly newCanvas: () => void
  readonly newDirectory: () => void
  readonly importFile: () => void
  readonly refresh: () => void
}

export function ExplorerToolbar(props: Props) {
  const { disabled } = props
  return (
    <div className="flex items-center justify-between px-3 pb-2">
      <span className="text-xs font-medium text-muted-foreground whitespace-nowrap shrink-0">
        画布与目录
      </span>
      <div className="flex">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          title="新建画布"
          aria-label="新建画布"
          disabled={disabled}
          onClick={props.newCanvas}
        >
          <Plus size={16} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          title="新建目录"
          aria-label="新建目录"
          disabled={disabled}
          onClick={props.newDirectory}
        >
          <FolderPlus size={16} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          title="导入画布"
          aria-label="导入画布"
          disabled={disabled}
          onClick={props.importFile}
        >
          <Upload size={15} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          title="刷新文件"
          aria-label="刷新文件"
          disabled={disabled}
          onClick={props.refresh}
        >
          <RefreshCw size={14} />
        </Button>
      </div>
    </div>
  )
}
