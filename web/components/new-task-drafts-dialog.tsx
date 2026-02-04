"use client"

import { Trash2 } from "lucide-react"

import type { NewTaskDraft } from "@/lib/new-task-drafts"
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog"

function formatDraftTitle(text: string): string {
  const firstLine = text.split("\n")[0] ?? ""
  const trimmed = firstLine.trim()
  if (!trimmed) return "Untitled draft"
  return trimmed.length > 72 ? `${trimmed.slice(0, 72)}…` : trimmed
}

export function NewTaskDraftsDialog({
  open,
  onOpenChange,
  drafts,
  onOpenDraft,
  onDeleteDraft,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
  drafts: NewTaskDraft[]
  onOpenDraft: (draft: NewTaskDraft) => void
  onDeleteDraft: (draftId: string) => void
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[560px]" data-testid="new-task-drafts-dialog">
        <DialogHeader>
          <DialogTitle>Drafts</DialogTitle>
        </DialogHeader>

        {drafts.length === 0 ? (
          <div className="text-sm text-muted-foreground">No drafts saved.</div>
        ) : (
          <div className="max-h-[420px] overflow-y-auto border rounded-md">
            {drafts.map((draft, idx) => (
              <div
                key={draft.id}
                data-testid={`new-task-drafts-row-${idx}`}
                data-draft-id={draft.id}
                className="flex items-center justify-between gap-2 px-3 py-2 border-b last:border-b-0 hover:bg-muted/30"
              >
                <button
                  type="button"
                  className="flex-1 text-left min-w-0"
                  data-testid={`new-task-drafts-open-${idx}`}
                  onClick={() => onOpenDraft(draft)}
                >
                  <div className="text-sm font-medium truncate">{formatDraftTitle(draft.text)}</div>
                  <div className="text-xs text-muted-foreground truncate">
                    Updated {new Date(draft.updatedAtUnixMs).toLocaleString()}
                  </div>
                </button>
                <button
                  type="button"
                  className="w-8 h-8 rounded-md hover:bg-muted flex items-center justify-center"
                  data-testid={`new-task-drafts-delete-${idx}`}
                  title="Delete draft"
                  onClick={() => onDeleteDraft(draft.id)}
                >
                  <Trash2 className="w-4 h-4" />
                </button>
              </div>
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
