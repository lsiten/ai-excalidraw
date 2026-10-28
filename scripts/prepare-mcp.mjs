import { writeFile } from 'node:fs/promises'
import { tools, instructions } from './mcp-tools.mjs'
await writeFile(new URL('../src-tauri/src/mcp-tools.json', import.meta.url), `${JSON.stringify({ tools, instructions })}\n`)
