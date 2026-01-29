"use client"

import { useEffect, useRef, useState } from "react"
import Image from "next/image"
import { Dialog, DialogContent } from "@/components/ui/dialog"
import { Button } from "@/components/ui/button"
import { useLuban } from "@/lib/luban-context"
import { uploadAttachment } from "@/lib/luban-http"
import { focusChatInput } from "@/lib/focus-chat-input"
import type { AttachmentRef, FeedbackSubmitAction, FeedbackType, TaskIssueInfo } from "@/lib/luban-api"
import {
  Bug,
  Lightbulb,
  HelpCircle,
  ChevronLeft,
  Loader2,
  Send,
  CheckCircle2,
  Sparkles,
  ImagePlus,
  X,
  Play,
} from "lucide-react"
import { cn } from "@/lib/utils"
import { toast } from "sonner"
import { getBuildInfo } from "@/lib/build-info"

type Stage = "input" | "review"

interface SystemInfo {
  version: string
  commit: string
  tag: string
  buildTime: string
  channel: string
  os: string
  runtime: string
}

interface AnalyzedIssue {
  title: string
  type: FeedbackType
  labels: string[]
  body: string
}

interface Attachment {
  id: string
  name: string
  url: string
  file: File
  type: "image"
}

function getSystemInfo(): SystemInfo {
  const build = getBuildInfo()
  const runtime =
    typeof window !== "undefined" && typeof (window as unknown as { __TAURI__?: unknown }).__TAURI__ !== "undefined"
      ? "Tauri"
      : "Web"
  const os = typeof navigator !== "undefined" ? `${navigator.platform} (${navigator.userAgent})` : "Unknown"
  return {
    version: build.version,
    commit: build.commit,
    tag: build.tag,
    buildTime: build.buildTime,
    channel: build.channel,
    os,
    runtime,
  }
}

function collapseWhitespace(input: string): string {
  return input.replace(/\s+/g, " ").trim()
}

function summarizeForTitle(input: string): string {
  const firstLine = input
    .split("\n")
    .map((l) => l.trim())
    .find((l) => l.length > 0) ?? ""
  const compact = collapseWhitespace(firstLine)
  if (!compact) return "Feedback"
  const maxLen = 72
  if (compact.length <= maxLen) return compact
  return `${compact.slice(0, maxLen - 1)}…`
}

function detectFeedbackType(input: string): FeedbackType {
  const lower = input.toLowerCase()
  const bugKeywords = [
    "bug",
    "error",
    "crash",
    "broken",
    "not working",
    "fail",
    "issue",
    "problem",
    "wrong",
    "stuck",
    "freeze",
    "doesn't work",
    "can't",
    "cannot",
  ]
  const featureKeywords = [
    "feature",
    "add",
    "support",
    "would be nice",
    "request",
    "suggest",
    "improve",
    "enhancement",
    "wish",
  ]

  const hasBugKeyword = bugKeywords.some((k) => lower.includes(k))
  const hasFeatureKeyword = featureKeywords.some((k) => lower.includes(k))

  if (hasBugKeyword && !hasFeatureKeyword) return "bug"
  if (hasFeatureKeyword && !hasBugKeyword) return "feature"
  if (hasBugKeyword && hasFeatureKeyword) return "bug"
  return "question"
}

function analyzeFeedback(args: { input: string; systemInfo: SystemInfo; attachmentCount: number }): AnalyzedIssue {
  const inputTrimmed = args.input.trim()
  const detectedType = detectFeedbackType(inputTrimmed)
  const titleBase = summarizeForTitle(inputTrimmed)
  const tagLine = args.systemInfo.tag ? `- Tag: ${args.systemInfo.tag}\n` : ""

  if (detectedType === "bug") {
    return {
      title: `Bug: ${titleBase}`,
      type: "bug",
      labels: ["bug"],
      body: `## Description
${inputTrimmed}

## Steps to Reproduce
1. ...
2. ...
3. ...

## Expected Behavior
...

## Actual Behavior
...

## Attachments
${args.attachmentCount > 0 ? `${args.attachmentCount} image(s) attached via Luban feedback.` : "None"}

## System Information
- Version: ${args.systemInfo.version}
- Channel: ${args.systemInfo.channel}
- Commit: ${args.systemInfo.commit}
${tagLine}- Build Time: ${args.systemInfo.buildTime}
- OS: ${args.systemInfo.os}
- Runtime: ${args.systemInfo.runtime}`,
    }
  }

  if (detectedType === "feature") {
    return {
      title: `Feature: ${titleBase}`,
      type: "feature",
      labels: ["enhancement"],
      body: `## Feature Request
${inputTrimmed}

## Motivation
...

## Proposed Solution
...

## System Information
- Version: ${args.systemInfo.version}
- Channel: ${args.systemInfo.channel}
- Commit: ${args.systemInfo.commit}
${tagLine}- Build Time: ${args.systemInfo.buildTime}
- OS: ${args.systemInfo.os}
- Runtime: ${args.systemInfo.runtime}`,
    }
  }

  return {
    title: `Question: ${titleBase}`,
    type: "question",
    labels: ["question"],
    body: `## Question
${inputTrimmed}

## System Information
- Version: ${args.systemInfo.version}
- Channel: ${args.systemInfo.channel}
- Commit: ${args.systemInfo.commit}
${tagLine}- Build Time: ${args.systemInfo.buildTime}
- OS: ${args.systemInfo.os}
- Runtime: ${args.systemInfo.runtime}`,
  }
}

const typeConfig = {
  bug: {
    icon: Bug,
    label: "Bug Report",
    color: "text-status-error",
    bgColor: "bg-status-error/10",
  },
  feature: {
    icon: Lightbulb,
    label: "Feature Request",
    color: "text-status-success",
    bgColor: "bg-status-success/10",
  },
  question: {
    icon: HelpCircle,
    label: "Question",
    color: "text-status-info",
    bgColor: "bg-status-info/10",
  },
} satisfies Record<
  FeedbackType,
  {
    icon: typeof Bug
    label: string
    color: string
    bgColor: string
  }
>

interface FeedbackModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function FeedbackModal({ open, onOpenChange }: FeedbackModalProps) {
  const { openWorkspace, sendAgentMessageTo, submitFeedback } = useLuban()
  const [stage, setStage] = useState<Stage>("input")
  const [input, setInput] = useState("")
  const [attachments, setAttachments] = useState<Attachment[]>([])
  const [isPolishing, setIsPolishing] = useState(false)
  const [analyzedIssue, setAnalyzedIssue] = useState<AnalyzedIssue | null>(null)
  const [editedTitle, setEditedTitle] = useState("")
  const [editedBody, setEditedBody] = useState("")
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [submitAction, setSubmitAction] = useState<FeedbackSubmitAction | null>(null)
  const [isSubmitted, setIsSubmitted] = useState(false)
  const [submittedIssue, setSubmittedIssue] = useState<TaskIssueInfo | null>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const seqRef = useRef(0)

  const inputTrimmed = input.trim()
  const hasInput = inputTrimmed.length > 5

  const revokeAttachmentUrls = (items: Attachment[]) => {
    for (const att of items) {
      try {
        URL.revokeObjectURL(att.url)
      } catch {
        // Ignore.
      }
    }
  }

  useEffect(() => {
    if (open) return
    const t = window.setTimeout(() => {
      setStage("input")
      setInput("")
      revokeAttachmentUrls(attachments)
      setAttachments([])
      setAnalyzedIssue(null)
      setEditedTitle("")
      setEditedBody("")
      setIsSubmitted(false)
      setSubmitAction(null)
      setSubmittedIssue(null)
      setIsPolishing(false)
      setIsSubmitting(false)
      seqRef.current += 1
    }, 200)
    return () => window.clearTimeout(t)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  useEffect(() => {
    if (!open) return
    const t = window.setTimeout(() => textareaRef.current?.focus(), 50)
    return () => window.clearTimeout(t)
  }, [open])

  const handlePolish = async () => {
    if (!hasInput) return

    const seq = (seqRef.current += 1)
    setIsPolishing(true)
    await new Promise((resolve) => window.setTimeout(resolve, 1200))
    if (seqRef.current !== seq) return

    const systemInfo = getSystemInfo()
    const issue = analyzeFeedback({ input: inputTrimmed, systemInfo, attachmentCount: attachments.length })
    setAnalyzedIssue(issue)
    setEditedTitle(issue.title)
    setEditedBody(issue.body)
    setIsPolishing(false)
    setStage("review")
  }

  const handleBack = () => {
    setStage("input")
  }

  const handleSubmit = async (action: FeedbackSubmitAction) => {
    if (!analyzedIssue) return
    const title = editedTitle.trim()
    if (!title) return

    const seq = (seqRef.current += 1)
    setSubmitAction(action)
    setIsSubmitting(true)
    try {
      const result = await submitFeedback({
        title,
        body: editedBody.trimEnd(),
        labels: analyzedIssue.labels,
        feedbackType: analyzedIssue.type,
        action,
      })
      if (seqRef.current !== seq) return

      setSubmittedIssue(result.issue)

      if (action === "fix_it") {
        const task = result.task
        if (!task) throw new Error("server returned no task result")

        await openWorkspace(task.workspace_id)

        const settled = await Promise.allSettled(
          attachments.map((att) =>
            uploadAttachment({ workspaceId: task.workspace_id, file: att.file, kind: "image" }),
          ),
        )

        const uploaded: AttachmentRef[] = []
        for (const entry of settled) {
          if (entry.status === "fulfilled") uploaded.push(entry.value)
        }

        sendAgentMessageTo(task.workspace_id, task.thread_id, task.prompt, uploaded)
        focusChatInput()
        toast("Task started")
      } else {
        toast("Issue created")
      }

      setIsSubmitted(true)
      window.setTimeout(() => onOpenChange(false), 1500)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err))
      setSubmitAction(null)
    } finally {
      if (seqRef.current === seq) setIsSubmitting(false)
    }
  }

  const handlePaste = (e: React.ClipboardEvent) => {
    const items = e.clipboardData.items
    let sawImage = false
    for (const item of Array.from(items)) {
      if (!item.type.startsWith("image/")) continue
      const file = item.getAsFile()
      if (!file) continue
      sawImage = true
      const url = URL.createObjectURL(file)
      setAttachments((prev) => [
        ...prev,
        { id: crypto.randomUUID(), name: file.name || "screenshot.png", url, file, type: "image" },
      ])
    }

    if (sawImage) e.preventDefault()
  }

  const removeAttachment = (id: string) => {
    setAttachments((prev) => {
      const removed = prev.find((a) => a.id === id) ?? null
      if (removed) revokeAttachmentUrls([removed])
      return prev.filter((a) => a.id !== id)
    })
  }

  const detectedType = analyzedIssue ? typeConfig[analyzedIssue.type] : null

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        data-testid="feedback-modal"
        className="sm:max-w-[560px] p-0 gap-0 bg-background border-border overflow-hidden"
      >
        <div className="px-5 py-4 border-b border-border flex items-center gap-3">
          {stage === "review" && (
            <button
              type="button"
              onClick={handleBack}
              className="p-1 -ml-1 text-muted-foreground hover:text-foreground hover:bg-secondary rounded transition-colors"
            >
              <ChevronLeft className="w-4 h-4" />
            </button>
          )}
          <h2 className="text-base font-medium flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-primary" />
            {stage === "input" ? "Send Feedback" : "Review Issue"}
          </h2>
        </div>

        {isSubmitted ? (
          <div className="p-8 flex flex-col items-center justify-center text-center">
            <div className="w-12 h-12 rounded-full bg-status-success/10 flex items-center justify-center mb-4">
              <CheckCircle2 className="w-6 h-6 text-status-success" />
            </div>
            <h3 className="text-sm font-medium mb-1">
              {submitAction === "fix_it" ? "Task Started!" : "Issue Created!"}
            </h3>
            <p className="text-xs text-muted-foreground">
              {submitAction === "fix_it" ? (
                "Agent is working on fixing this issue."
              ) : (
                <>
                  Issue{" "}
                  <span className="font-mono text-primary">#{submittedIssue?.number ?? "?"}</span>{" "}
                  has been created.
                </>
              )}
            </p>
          </div>
        ) : stage === "input" ? (
          <>
            <div className="p-5 space-y-4">
              <div className="relative rounded-lg border border-border hover:border-muted-foreground/30 focus-within:border-primary focus-within:ring-2 focus-within:ring-primary/20 transition-all duration-200">
                <textarea
                  ref={textareaRef}
                  value={input}
                  onChange={(e) => setInput(e.target.value)}
                  onPaste={handlePaste}
                  placeholder="Describe the bug or feature you'd like..."
                  className="w-full min-h-[100px] p-4 bg-transparent text-sm resize-none placeholder:text-muted-foreground/50 focus:outline-none"
                  disabled={isPolishing}
                  autoFocus
                />

                {attachments.length > 0 && (
                  <div className="px-4 pb-3 flex flex-wrap gap-2">
                    {attachments.map((att) => (
                      <div
                        key={att.id}
                        className="relative group w-16 h-16 rounded-lg overflow-hidden border border-border"
                      >
                        <Image
                          src={att.url}
                          alt={att.name}
                          width={64}
                          height={64}
                          className="w-full h-full object-cover"
                          unoptimized
                        />
                        <button
                          type="button"
                          onClick={() => removeAttachment(att.id)}
                          className="absolute top-0.5 right-0.5 p-0.5 bg-background/80 rounded-full opacity-0 group-hover:opacity-100 transition-opacity"
                        >
                          <X className="w-3 h-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>

            <div className="px-5 py-4 border-t border-border bg-secondary/30 flex items-center justify-end gap-2">
              <Button size="sm" onClick={handlePolish} disabled={!hasInput || isPolishing}>
                {isPolishing ? <Loader2 className="w-4 h-4 animate-spin" /> : <Sparkles className="w-4 h-4" />}
                {isPolishing ? "Polishing..." : "Polish"}
              </Button>
            </div>
          </>
        ) : (
          <div className="flex flex-col max-h-[70vh]">
            {detectedType && (
              <div className="px-5 pt-4 pb-2">
                <div
                  className={cn(
                    "inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium",
                    detectedType.bgColor,
                    detectedType.color,
                  )}
                >
                  <detectedType.icon className="w-3.5 h-3.5" />
                  {detectedType.label}
                </div>
              </div>
            )}

            <div className="px-5 pb-3">
              <input
                type="text"
                value={editedTitle}
                onChange={(e) => setEditedTitle(e.target.value)}
                className="w-full text-base font-medium bg-transparent border-none focus:outline-none focus:ring-0 placeholder:text-muted-foreground"
                placeholder="Issue title..."
              />
              {analyzedIssue && (
                <div className="flex items-center gap-1.5 mt-2 flex-wrap">
                  {analyzedIssue.labels.map((label) => (
                    <span
                      key={label}
                      className="px-1.5 py-0.5 text-[10px] font-medium rounded bg-secondary text-muted-foreground"
                    >
                      {label}
                    </span>
                  ))}
                </div>
              )}
            </div>

            <div className="flex-1 overflow-hidden border-t border-border">
              <div className="h-full max-h-[300px] overflow-y-auto">
                <textarea
                  value={editedBody}
                  onChange={(e) => setEditedBody(e.target.value)}
                  className="w-full h-full min-h-[200px] p-5 bg-transparent text-sm font-mono resize-none placeholder:text-muted-foreground/50 focus:outline-none"
                  placeholder="Issue body..."
                />
              </div>
            </div>

            {attachments.length > 0 && (
              <div className="px-5 py-3 border-t border-border">
                <div className="flex items-center gap-2 text-xs text-muted-foreground mb-2">
                  <ImagePlus className="w-3 h-3" />
                  {attachments.length} attachment{attachments.length > 1 ? "s" : ""}
                </div>
                <div className="flex flex-wrap gap-2">
                  {attachments.map((att) => (
                    <div key={att.id} className="w-12 h-12 rounded overflow-hidden border border-border">
                      <Image
                        src={att.url}
                        alt={att.name}
                        width={48}
                        height={48}
                        className="w-full h-full object-cover"
                        unoptimized
                      />
                    </div>
                  ))}
                </div>
              </div>
            )}

            <div className="px-5 py-4 border-t border-border bg-secondary/30 flex items-center justify-end gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => void handleSubmit("create_issue")}
                disabled={isSubmitting || !editedTitle.trim()}
              >
                {isSubmitting && submitAction === "create_issue" ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Send className="w-4 h-4" />
                )}
                {isSubmitting && submitAction === "create_issue" ? "Creating..." : "Create Issue"}
              </Button>
              <Button
                size="sm"
                onClick={() => void handleSubmit("fix_it")}
                disabled={isSubmitting || !editedTitle.trim()}
              >
                {isSubmitting && submitAction === "fix_it" ? (
                  <Loader2 className="w-4 h-4 animate-spin" />
                ) : (
                  <Play className="w-4 h-4" />
                )}
                {isSubmitting && submitAction === "fix_it" ? "Starting..." : "Fix it"}
              </Button>
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
