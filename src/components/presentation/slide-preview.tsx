import { useEffect, useRef, useState } from 'react'
import { Loader2, AlertCircle } from 'lucide-react'
import { renderSlide } from '@/lib/presentation/render'
import type { PlaybackArea, Slide } from '@/lib/presentation/types'
import { message } from '@/lib/workspace/types'

export function SlidePreview({
  projectId,
  path,
  area,
  className,
}: {
  readonly projectId: string
  readonly path: string
  readonly area?: PlaybackArea
  readonly className?: string
}) {
  const root = useRef<HTMLDivElement>(null)
  const [visible, setVisible] = useState(false)
  const key = JSON.stringify([projectId, path, area])
  const [result, setResult] = useState<{
    key: string
    slide: Slide | null
    error: string | null
  } | null>(null)
  const slide = result?.key === key ? result.slide : null
  const error = result?.key === key ? result.error : null
  useEffect(() => {
    const node = root.current
    if (!node) return
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        setVisible(true)
        observer.disconnect()
      }
    })
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  useEffect(() => {
    if (!visible) return
    let disposed = false,
      url: string | undefined
    void renderSlide(projectId, path, area)
      .then((next) => {
        url = next.url
        if (disposed) URL.revokeObjectURL(next.url)
        else setResult({ key, slide: next, error: null })
      })
      .catch((error) => {
        if (!disposed) setResult({ key, slide: null, error: message(error) })
      })
    return () => {
      disposed = true
      if (url) URL.revokeObjectURL(url)
    }
  }, [projectId, path, area, visible, key])
  return (
    <div
      ref={root}
      className={`flex items-center justify-center overflow-hidden ${className ?? ''}`}
    >
      {slide ? (
        <img
          src={slide.url}
          alt={path.replace(/\.excalidraw$/, '')}
          className="h-full w-full object-contain"
          draggable={false}
        />
      ) : error ? (
        <span className="flex items-center gap-1 p-2 text-xs text-destructive" title={error}>
          <AlertCircle size={14} />
          预览失败
        </span>
      ) : (
        <Loader2
          size={18}
          className="animate-spin text-muted-foreground"
          aria-label="正在生成预览"
        />
      )}
    </div>
  )
}
