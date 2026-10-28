/**
 * localStorage 读写封装：防抖写入与配额错误上报
 */
import { persistDesktopCanvas, removeDesktopCanvas } from './desktop-storage'

export class StorageQuotaError extends Error {
  constructor(message = 'localStorage 配额不足') {
    super(message)
    this.name = 'StorageQuotaError'
  }
}

type ErrorListener = (error: Error) => void

const errorListeners = new Set<ErrorListener>()

export function onStorageError(listener: ErrorListener): () => void {
  errorListeners.add(listener)
  return () => errorListeners.delete(listener)
}

function notifyError(error: Error) {
  errorListeners.forEach((listener) => {
    try {
      listener(error)
    } catch {
      // ignore listener errors
    }
  })
}

export function readStorage<T>(key: string, fallback: T): T {
  if (typeof window === 'undefined') return fallback
  try {
    const data = localStorage.getItem(key)
    if (!data) return fallback
    return JSON.parse(data) as T
  } catch {
    return fallback
  }
}

export function writeStorage(key: string, value: unknown): boolean {
  if (typeof window === 'undefined') return false
  try {
    localStorage.setItem(key, JSON.stringify(value))
    void persistDesktopCanvas(key, value).catch((error: unknown) => notifyError(error instanceof Error ? error : new Error(String(error))))
    return true
  } catch (error) {
    const err =
      error instanceof DOMException &&
      (error.name === 'QuotaExceededError' || error.code === 22)
        ? new StorageQuotaError()
        : error instanceof Error
          ? error
          : new Error(String(error))
    console.warn(`Failed to write ${key}:`, err)
    notifyError(err)
    return false
  }
}

export function removeStorage(key: string): void {
  if (typeof window === 'undefined') return
  try {
    localStorage.removeItem(key)
    void removeDesktopCanvas(key).catch((error: unknown) => notifyError(error instanceof Error ? error : new Error(String(error))))
  } catch {
    // ignore
  }
}

interface DebouncedWriter {
  write: (value: unknown) => void
  flush: () => void
  cancel: () => void
}

export function createDebouncedWriter(
  key: string,
  delayMs = 500
): DebouncedWriter {
  let timer: ReturnType<typeof setTimeout> | null = null
  let pending: unknown | undefined
  let hasPending = false

  const flush = () => {
    if (timer) {
      clearTimeout(timer)
      timer = null
    }
    if (!hasPending) return
    writeStorage(key, pending)
    hasPending = false
    pending = undefined
  }

  const write = (value: unknown) => {
    pending = value
    hasPending = true
    if (timer) clearTimeout(timer)
    timer = setTimeout(flush, delayMs)
  }

  const cancel = () => {
    if (timer) {
      clearTimeout(timer)
      timer = null
    }
    hasPending = false
    pending = undefined
  }

  return { write, flush, cancel }
}
