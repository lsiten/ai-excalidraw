import { cp, mkdir } from 'node:fs/promises'

const source = new URL('../node_modules/@excalidraw/excalidraw/dist/prod/fonts/', import.meta.url)
const target = new URL('../public/fonts/', import.meta.url)
await mkdir(target, { recursive: true })
await cp(source, target, { recursive: true })
