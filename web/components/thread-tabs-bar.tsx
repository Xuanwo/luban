"use client"

import { useState } from "react"
import { ChevronDown, GitCompareArrows, MessageSquare, Plus, RotateCcw, X } from "lucide-react"

import { cn } from "@/lib/utils"
import type { ArchivedTab, ChatTab } from "@/lib/use-thread-tabs"

export function ThreadTabsBar(props: {
  tabs: ChatTab[]
  archivedTabs: ArchivedTab[]
  activeTabId: string
  activePanel: "thread" | "diff"
  isDiffTabOpen: boolean
  onTabClick: (tabId: string) => void
  onCloseTab: (tabId: string, e: React.MouseEvent) => void
  onAddTab: () => void
  onDiffTabClick: () => void
  onCloseDiffTab: (e: React.MouseEvent) => void
  onRestoreTab: (tab: ArchivedTab) => void
}) {
  const [showTabDropdown, setShowTabDropdown] = useState(false)

  return (
    <div className="flex items-center h-11 border-b border-border bg-secondary px-2">
      <div className="flex-1 flex items-center gap-1 min-w-0 overflow-x-auto scrollbar-none py-1.5">
        {props.tabs.map((tab) => (
          <div
            key={tab.id}
            data-testid={`thread-tab-${tab.id}`}
            onClick={() => props.onTabClick(tab.id)}
            className={cn(
              "group relative flex items-center gap-2 h-8 px-3 cursor-pointer transition-all duration-200 min-w-0 max-w-[180px] rounded-lg",
              props.activePanel === "thread" && tab.id === props.activeTabId
                ? "text-foreground bg-glass-surface shadow-lg"
                : "text-muted-foreground hover:text-foreground hover:bg-glass-surface-muted/50",
            )}
          >
            <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
            <span data-testid="thread-tab-title" className="text-xs truncate flex-1">
              {tab.title}
            </span>
            {props.tabs.length > 1 && (
              <button
                onClick={(e) => props.onCloseTab(tab.id, e)}
                className="p-0.5 opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto hover:bg-muted rounded transition-all"
              >
                <X className="w-3 h-3" />
              </button>
            )}
          </div>
        ))}

        {props.isDiffTabOpen && (
          <div
            key="diff-tab"
            onClick={props.onDiffTabClick}
            className={cn(
              "group relative flex items-center gap-2 h-8 px-3 cursor-pointer transition-all duration-200 min-w-0 max-w-[180px] rounded-lg",
              props.activePanel === "diff"
                ? "text-foreground bg-glass-surface shadow-lg"
                : "text-muted-foreground hover:text-foreground hover:bg-glass-surface-muted/50",
            )}
          >
            <GitCompareArrows className="w-3.5 h-3.5 flex-shrink-0" />
            <span className="text-xs truncate flex-1">Changes</span>
            <button
              onClick={props.onCloseDiffTab}
              className="p-0.5 opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto hover:bg-muted rounded transition-all"
              title="Close changes tab"
            >
              <X className="w-3 h-3" />
            </button>
          </div>
        )}

        <button
          onClick={props.onAddTab}
          className="flex items-center justify-center w-8 h-10 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors flex-shrink-0"
          title="New tab"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      <div className="flex items-center px-1">
        <div className="relative">
          <button
            onClick={() => setShowTabDropdown(!showTabDropdown)}
            className={cn(
              "flex items-center justify-center w-8 h-8 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors",
              showTabDropdown && "bg-muted text-foreground",
            )}
            title="All tabs"
          >
            <ChevronDown className="w-4 h-4" />
          </button>

          {showTabDropdown && (
            <>
              <div className="fixed inset-0 z-40" onClick={() => setShowTabDropdown(false)} />
              <div className="absolute right-0 top-full mt-1 w-64 bg-card border border-border rounded-lg shadow-xl z-50 overflow-hidden">
                <div className="p-2 border-b border-border">
                  <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                    Open Tabs
                  </span>
                </div>
                <div className="max-h-40 overflow-y-auto">
                  {props.isDiffTabOpen && (
                    <button
                      onClick={() => {
                        props.onDiffTabClick()
                        setShowTabDropdown(false)
                      }}
                      className={cn(
                        "w-full flex items-center gap-2 px-3 py-2 text-left text-xs hover:bg-muted transition-colors",
                        props.activePanel === "diff" && "bg-primary/10 text-primary",
                      )}
                    >
                      <GitCompareArrows className="w-3.5 h-3.5 flex-shrink-0" />
                      <span className="truncate">Changes</span>
                    </button>
                  )}
                  {props.tabs.map((tab) => (
                    <button
                      key={tab.id}
                      onClick={() => {
                        props.onTabClick(tab.id)
                        setShowTabDropdown(false)
                      }}
                      className={cn(
                        "w-full flex items-center gap-2 px-3 py-2 text-left text-xs hover:bg-muted transition-colors",
                        props.activePanel === "thread" && tab.id === props.activeTabId && "bg-primary/10 text-primary",
                      )}
                    >
                      <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
                      <span className="truncate">{tab.title}</span>
                    </button>
                  ))}
                </div>

                {props.archivedTabs.length > 0 && (
                  <>
                    <div className="p-2 border-t border-border">
                      <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                        Recently Closed
                      </span>
                    </div>
                    <div className="max-h-32 overflow-y-auto">
                      {props.archivedTabs.map((tab) => (
                        <button
                          key={tab.id}
                          onClick={() => {
                            props.onRestoreTab(tab)
                            setShowTabDropdown(false)
                          }}
                          className="w-full flex items-center gap-2 px-3 py-2 text-left text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                        >
                          <RotateCcw className="w-3.5 h-3.5 flex-shrink-0" />
                          <span className="truncate flex-1">{tab.title}</span>
                        </button>
                      ))}
                    </div>
                  </>
                )}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
