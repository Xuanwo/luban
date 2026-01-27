"use client"

import { useState, useEffect } from "react"
import { ChevronDown, Check, Copy, FolderOpen } from "lucide-react"
import { cn } from "@/lib/utils"
import Image from "next/image"

// Brand icon component using official logos
function BrandIcon({ src, alt, className }: { src: string; alt: string; className?: string }) {
  return (
    <Image
      src={src}
      alt={alt}
      width={12}
      height={12}
      className={className}
      style={{ width: 12, height: 12 }}
    />
  )
}

export type EditorType = "vscode" | "cursor" | "zed"
export type ActionType = "copy-path" | "ghostty" | "finder"

interface EditorConfig {
  id: EditorType
  name: string
  icon: string
}

interface ActionConfig {
  id: ActionType
  name: string
  icon: string | React.ComponentType<{ className?: string }>
}

const editors: EditorConfig[] = [
  { id: "vscode", name: "VS Code", icon: "/icons/vscode.svg" },
  { id: "cursor", name: "Cursor", icon: "/icons/cursor.svg" },
  { id: "zed", name: "Zed", icon: "/icons/zed.svg" },
]

const actions: ActionConfig[] = [
  { id: "ghostty", name: "Ghostty", icon: "/icons/ghostty.png" },
  { id: "copy-path", name: "Copy Path", icon: Copy },
  { id: "finder", name: "Reveal in Finder", icon: FolderOpen },
]

type SelectedItem = { type: "editor"; id: EditorType } | { type: "action"; id: ActionType }

const STORAGE_KEY = "open-button-last-selection"

function getDefaultSelection(): SelectedItem {
  return { type: "editor", id: "vscode" }
}

function getStoredSelection(): SelectedItem {
  if (typeof window === "undefined") return getDefaultSelection()
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      if (parsed.type === "editor" && editors.some((e) => e.id === parsed.id)) {
        return parsed
      }
      if (parsed.type === "action" && actions.some((a) => a.id === parsed.id)) {
        return parsed
      }
    }
  } catch {
    // ignore
  }
  return getDefaultSelection()
}

function saveSelection(selection: SelectedItem) {
  if (typeof window === "undefined") return
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(selection))
  } catch {
    // ignore
  }
}

function getItemConfig(selection: SelectedItem): { name: string; icon: string | React.ComponentType<{ className?: string }> } {
  if (selection.type === "editor") {
    const editor = editors.find((e) => e.id === selection.id)
    return editor || editors[0]
  }
  const action = actions.find((a) => a.id === selection.id)
  return action || actions[0]
}

function renderIcon(icon: string | React.ComponentType<{ className?: string }>, className?: string) {
  if (typeof icon === "string") {
    return <BrandIcon src={icon} alt="" className={className} />
  }
  const Icon = icon
  return <Icon className={className} />
}

export function OpenButton() {
  const [selection, setSelection] = useState<SelectedItem>(getDefaultSelection)
  const [isOpen, setIsOpen] = useState(false)
  const [copied, setCopied] = useState(false)

  useEffect(() => {
    setSelection(getStoredSelection())
  }, [])

  const handleSelect = (item: SelectedItem) => {
    setSelection(item)
    saveSelection(item)
    setIsOpen(false)
    executeAction(item)
  }

  const handlePrimaryClick = () => {
    executeAction(selection)
  }

  const executeAction = (item: SelectedItem) => {
    if (item.type === "action" && item.id === "copy-path") {
      setCopied(true)
      setTimeout(() => setCopied(false), 1500)
    }
    console.log("Execute:", item)
  }

  const config = getItemConfig(selection)

  return (
    <div className="relative inline-flex">
      {/* Primary action - click to execute */}
      <button
        onMouseDown={(e) => e.preventDefault()}
        onClick={handlePrimaryClick}
        className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors rounded-l px-1"
      >
        {copied && selection.type === "action" && selection.id === "copy-path" ? (
          <Check className="w-3 h-3 text-status-success flex-shrink-0" />
        ) : (
          renderIcon(config.icon, "w-3 h-3 flex-shrink-0")
        )}
        <span className="text-xs">{config.name}</span>
      </button>

      {/* Dropdown trigger */}
      <button
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => setIsOpen(!isOpen)}
        className={cn(
          "inline-flex items-center justify-center w-5 py-1 text-muted-foreground hover:text-foreground hover:bg-muted transition-colors rounded-r",
          isOpen && "bg-muted text-foreground"
        )}
      >
        <ChevronDown className={cn("w-3 h-3 transition-transform", isOpen && "rotate-180")} />
      </button>

      {/* Dropdown menu */}
      {isOpen && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setIsOpen(false)} />
          <div className="absolute right-0 top-full mt-1 z-50 w-44 bg-popover border border-border rounded-lg shadow-xl overflow-hidden">
            <div className="p-1">
              {editors.map((editor) => (
                <button
                  key={editor.id}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => handleSelect({ type: "editor", id: editor.id })}
                  className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap text-foreground hover:bg-accent"
                >
                  {renderIcon(editor.icon, "w-3.5 h-3.5 flex-shrink-0")}
                  <span className="flex-1">{editor.name}</span>
                </button>
              ))}
            </div>
            <div className="border-t border-border" />
            <div className="p-1">
              <button
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect({ type: "action", id: "ghostty" })}
                className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap text-foreground hover:bg-accent"
              >
                {renderIcon("/icons/ghostty.png", "w-3.5 h-3.5 flex-shrink-0")}
                <span className="flex-1">Ghostty</span>
              </button>
            </div>
            <div className="border-t border-border" />
            <div className="p-1">
              {actions.filter(a => a.id !== "ghostty").map((action) => (
                <button
                  key={action.id}
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={() => handleSelect({ type: "action", id: action.id })}
                  className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left text-xs transition-colors rounded-md whitespace-nowrap text-foreground hover:bg-accent"
                >
                  {renderIcon(action.icon, "w-3.5 h-3.5 flex-shrink-0")}
                  <span className="flex-1">{action.name}</span>
                </button>
              ))}
            </div>
          </div>
        </>
      )}
    </div>
  )
}
