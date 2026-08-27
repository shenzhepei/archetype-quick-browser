import React from 'react'
import ReactDOM from 'react-dom/client'
import { I18nextProvider } from 'react-i18next'
import i18n from './i18n.js'
import { App } from './App.js'
import './styles.scss'

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><I18nextProvider i18n={i18n}><App /></I18nextProvider></React.StrictMode>)
