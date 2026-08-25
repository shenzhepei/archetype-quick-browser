import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './app/App'
import './i18n'
import './styles/main.scss'

document.documentElement.dataset.platform = window.archetype?.platform ?? 'web'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
)
