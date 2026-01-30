"use client"

import { ReactNode } from "react"

interface LubanLayoutProps {
  sidebar: ReactNode
  children: ReactNode
}

/**
 * Luban main layout with floating content panel
 * - Sidebar: 244px width, #f5f5f5 background
 * - Main content: #fcfcfc background, margin 8px 8px 8px 0, border-radius 4px
 */
export function LubanLayout({ sidebar, children }: LubanLayoutProps) {
  return (
    <div className="h-screen flex overflow-hidden" style={{ backgroundColor: '#f5f5f5' }}>
      {/* Left Sidebar */}
      <aside className="flex-shrink-0 flex flex-col overflow-hidden">
        {sidebar}
      </aside>

      {/* Main Content - Floating Panel */}
      <main
        className="flex-1 min-w-0 overflow-hidden"
        style={{
          margin: '8px 8px 8px 0',
          backgroundColor: '#fcfcfc',
          borderRadius: '4px',
          boxShadow: 'rgba(0, 0, 0, 0.022) 0px 3px 6px -2px, rgba(0, 0, 0, 0.044) 0px 1px 1px 0px'
        }}
      >
        {children}
      </main>
    </div>
  )
}
