"use client"

import { useEffect, useRef, useState } from "react"
import { Check, Copy, GitBranch, Loader2, Pencil, Sparkles } from "lucide-react"

import { OpenButton } from "@/components/shared/open-button"

export function WorkspaceChatHeader({
  projectName,
  branchName,
  isGit,
  isMainBranch,
  isBranchRenaming,
  onRenameBranch,
  onAiRenameBranch,
  onEditingBranchChange,
}: {
  projectName: string
  branchName: string
  isGit: boolean
  isMainBranch: boolean
  isBranchRenaming: boolean
  onRenameBranch: (nextBranch: string) => void
  onAiRenameBranch: () => void
  onEditingBranchChange?: (editing: boolean) => void
}) {
  const [isEditingBranch, setIsEditingBranch] = useState(false)
  const [branchValue, setBranchValue] = useState("")
  const [copySuccess, setCopySuccess] = useState(false)

  const branchInputRef = useRef<HTMLInputElement | null>(null)
  const branchRenameCanceledRef = useRef(false)

  useEffect(() => {
    if (!isEditingBranch) return
    const el = branchInputRef.current
    if (!el) return
    el.focus()
    el.select()
  }, [isEditingBranch])

  useEffect(() => {
    onEditingBranchChange?.(isEditingBranch)
  }, [isEditingBranch, onEditingBranchChange])

  const allowRenameButtons = isGit && !isMainBranch

  const submitRename = () => {
    if (!branchValue) return
    setIsEditingBranch(false)
    onRenameBranch(branchValue)
  }

  const cancelRename = () => {
    branchRenameCanceledRef.current = true
    setBranchValue(branchName)
    setIsEditingBranch(false)
  }

  return (
    <div className="flex items-center h-11 border-b border-border bg-card px-4">
      <div className="flex items-center gap-2 min-w-0">
        <span data-testid="active-project-name" className="text-sm font-medium text-foreground truncate">
          {projectName}
        </span>
        <div className="group/branch relative flex items-center gap-1 text-muted-foreground">
          <GitBranch className="w-3.5 h-3.5" />
          {isEditingBranch ? (
            <div className="flex items-center gap-1">
              <input
                ref={branchInputRef}
                type="text"
                value={branchValue}
                onChange={(e) => setBranchValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") submitRename()
                  if (e.key === "Escape") cancelRename()
                }}
                onBlur={() => {
                  setIsEditingBranch(false)
                  if (branchRenameCanceledRef.current) {
                    branchRenameCanceledRef.current = false
                    return
                  }
                  submitRename()
                }}
                className="text-xs bg-muted border border-border rounded px-1.5 py-0.5 w-40 focus:outline-none focus:ring-1 focus:ring-primary"
              />
              <button
                onMouseDown={(e) => e.preventDefault()}
                onClick={submitRename}
                className="p-0.5 text-muted-foreground hover:text-primary transition-colors"
                title="Confirm"
              >
                <Check className="w-3 h-3" />
              </button>
            </div>
          ) : (
            <>
              <span data-testid="active-workspace-branch" className="text-xs">
                {branchName}
              </span>
              {isBranchRenaming ? (
                <Loader2 className="w-3 h-3 animate-spin text-primary ml-1" />
              ) : (
                <div className="absolute right-0 top-1/2 -translate-y-1/2 z-10 flex items-center gap-0.5 opacity-0 group-hover/branch:opacity-100 transition-opacity bg-card px-0.5">
                  {allowRenameButtons && (
                    <>
                      <button
                        onClick={() => {
                          setBranchValue(branchName)
                          setIsEditingBranch(true)
                        }}
                        className="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                        title="Rename branch"
                      >
                        <Pencil className="w-3 h-3" />
                      </button>
                      <button
                        onClick={onAiRenameBranch}
                        className="p-0.5 text-muted-foreground hover:text-primary transition-colors"
                        title="AI rename"
                      >
                        <Sparkles className="w-3 h-3" />
                      </button>
                    </>
                  )}
                  <button
                    onClick={async () => {
                      if (!branchName) return
                      try {
                        await navigator.clipboard.writeText(branchName)
                        setCopySuccess(true)
                        window.setTimeout(() => setCopySuccess(false), 1200)
                      } catch {
                        setCopySuccess(false)
                      }
                    }}
                    className="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                    title={copySuccess ? "Copied!" : "Copy branch name"}
                  >
                    {copySuccess ? <Check className="w-3 h-3 text-status-success" /> : <Copy className="w-3 h-3" />}
                  </button>
                </div>
              )}
            </>
          )}
        </div>
        <OpenButton />
      </div>
    </div>
  )
}
