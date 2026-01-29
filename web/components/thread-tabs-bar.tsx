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

  const tabClassName = (isActive: boolean) =>
    cn(
      "group relative flex items-center gap-2 h-7 px-3 cursor-pointer transition-colors transition-shadow duration-200 min-w-0 max-w-[180px] rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50",
      isActive ? "z-10 text-foreground luban-float-glass" : "text-muted-foreground hover:text-foreground hover:bg-muted hover:shadow-sm",
    )

  return (
    <div className="flex items-center h-10 bg-background px-2 border-b border-border">
      <div className="flex-1 flex items-center gap-0.5 min-w-0 overflow-x-auto scrollbar-none py-1.5 pl-1">
        {props.tabs.map((tab) => (
          <div
            key={tab.id}
            data-testid={`thread-tab-${tab.id}`}
            onClick={() => props.onTabClick(tab.id)}
            className={tabClassName(props.activePanel === "thread" && tab.id === props.activeTabId)}
            onKeyDown={(e) => {
              if (e.key !== "Enter" && e.key !== " ") return
              e.preventDefault()
              props.onTabClick(tab.id)
            }}
            role="button"
            tabIndex={0}
            aria-current={props.activePanel === "thread" && tab.id === props.activeTabId ? "page" : undefined}
            data-active={props.activePanel === "thread" && tab.id === props.activeTabId ? "true" : "false"}
          >
            <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
            <span data-testid="thread-tab-title" className="text-xs truncate flex-1">
              {tab.title}
            </span>
            {props.tabs.length > 1 && (
              <button
                onClick={(e) => props.onCloseTab(tab.id, e)}
                className="absolute right-0 top-0 bottom-0 w-7 flex items-center justify-center bg-muted rounded-r-md opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto transition-opacity"
                type="button"
                aria-label="Close tab"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        ))}

        {props.isDiffTabOpen && (
          <div
            key="diff-tab"
            onClick={props.onDiffTabClick}
            className={tabClassName(props.activePanel === "diff")}
            onKeyDown={(e) => {
              if (e.key !== "Enter" && e.key !== " ") return
              e.preventDefault()
              props.onDiffTabClick()
            }}
            role="button"
            tabIndex={0}
            aria-current={props.activePanel === "diff" ? "page" : undefined}
            data-active={props.activePanel === "diff" ? "true" : "false"}
          >
            <GitCompareArrows className="w-3.5 h-3.5 flex-shrink-0" />
            <span className="text-xs truncate flex-1">Changes</span>
            <button
              onClick={props.onCloseDiffTab}
              className="absolute right-0 top-0 bottom-0 w-7 flex items-center justify-center bg-muted rounded-r-md opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto group-focus-within:opacity-100 group-focus-within:pointer-events-auto transition-opacity"
              title="Close changes tab"
              type="button"
              aria-label="Close changes tab"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        <button
          onClick={props.onAddTab}
          className="flex items-center justify-center w-8 h-10 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors flex-shrink-0"
          title="New tab"
          aria-label="New tab"
          type="button"
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
            aria-label="All tabs"
            type="button"
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
