import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import { restoreDesktopCanvases } from './lib/desktop-storage'

const root = document.getElementById('root')
if (root) {
  restoreDesktopCanvases().then(() => {
    createRoot(root).render(<StrictMode><App /></StrictMode>)
  }).catch((error: unknown) => {
    console.error('Cannot restore the local canvas:', error)
    root.textContent = '本地画布恢复失败，请保留应用数据并重新启动。'
  })
}
