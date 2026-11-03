import PptxGenJS from 'pptxgenjs'
import { WorkspaceError } from '@/lib/workspace/types'

export interface SlideImage {
  readonly path: string
  readonly data: string
  readonly width: number
  readonly height: number
}

export class PowerPointExport {
  private readonly document = new PptxGenJS()
  private pages = 0

  constructor(title: string) {
    this.document.layout = 'LAYOUT_WIDE'
    this.document.title = title
    this.document.author = 'AI Excalidraw'
  }

  add(image: SlideImage): void {
    const slide = this.document.addSlide()
    const scale = Math.min((40 / 3) / image.width, 7.5 / image.height)
    const w = image.width * scale, h = image.height * scale
    slide.addImage({ data: image.data, x: ((40 / 3) - w) / 2, y: (7.5 - h) / 2, w, h,
      altText: image.path.replace(/\.excalidraw$/, '') })
    slide.addNotes(`来源画布：${image.path}`)
    this.pages++
  }

  async write(): Promise<Blob> {
    if (!this.pages) throw new WorkspaceError('此文件夹没有可导出的画布')
    const output = await this.document.write({ outputType: 'blob', compression: true })
    if (!(output instanceof Blob)) throw new WorkspaceError('无法生成 PowerPoint 文件')
    return output
  }
}
