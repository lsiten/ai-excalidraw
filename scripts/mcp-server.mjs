#!/usr/bin/env node
// Compatibility stdio connector. The desktop process owns all MCP handlers.
import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join } from 'node:path'
import { createInterface } from 'node:readline'
const root = process.platform === 'darwin' ? join(homedir(), 'Library', 'Application Support')
  : process.platform === 'win32' ? process.env.APPDATA || join(homedir(), 'AppData', 'Roaming')
    : process.env.XDG_DATA_HOME || join(homedir(), '.local', 'share')
const path = process.env.AI_EXCALIDRAW_BRIDGE_FILE || join(root, 'com.local.ai-excalidraw', 'bridge.json')
try {
  const connection = JSON.parse(await readFile(path, 'utf8'))
  if (!Number.isInteger(connection.port) || connection.port < 1 || connection.port > 65535 || typeof connection.token !== 'string') throw new Error('Invalid desktop connection')
  const url = `http://127.0.0.1:${connection.port}/mcp`
  let checking = false
  const monitor = setInterval(async () => {
    if (checking) return
    checking = true
    try {
      const live = JSON.parse(await readFile(path, 'utf8'))
      if (live.token !== connection.token || live.pid !== connection.pid) throw new Error('Desktop session ended')
      const result = await fetch(url, { method: 'POST', redirect: 'error', headers: { Authorization: `Bearer ${connection.token}`, 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: 'health', method: 'ping' }), signal: AbortSignal.timeout(1500) })
      if (!result.ok) throw new Error('Desktop MCP stopped')
      await result.arrayBuffer()
    } catch (error) {
      console.error(error instanceof Error ? error.message : String(error))
      process.exit(0)
    } finally { checking = false }
  }, 1000)
  monitor.unref()
  try {
    for await (const line of createInterface({ input: process.stdin })) {
      if (!line.trim()) continue
      if (Buffer.byteLength(line) > 2097152) throw new Error('MCP frame is too large')
      const result = await fetch(url, {
        method: 'POST', redirect: 'error', signal: AbortSignal.timeout(20000),
        headers: { Authorization: `Bearer ${connection.token}`, 'Content-Type': 'application/json', Accept: 'application/json, text/event-stream' }, body: line,
      })
      if (!result.ok) throw new Error(`Desktop MCP returned HTTP ${result.status}; do not blindly retry mutations`)
      if (result.status !== 202) process.stdout.write(`${JSON.stringify(await result.json())}\n`)
    }
  } finally { clearInterval(monitor) }
} catch (error) {
  console.error(`Open AI Excalidraw desktop first. ${error instanceof Error ? error.message : String(error)}`)
  process.exitCode = 1
}
