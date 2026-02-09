"use client"

import { useCallback, useEffect, useRef } from "react"
import { useEditor, EditorContent } from "@tiptap/react"
import StarterKit from "@tiptap/starter-kit"
import { Markdown } from "@tiptap/markdown"
import { Table } from "@tiptap/extension-table"
import { TableRow } from "@tiptap/extension-table-row"
import { TableCell } from "@tiptap/extension-table-cell"
import { TableHeader } from "@tiptap/extension-table-header"
import { TaskList } from "@tiptap/extension-task-list"
import { TaskItem } from "@tiptap/extension-task-item"
import { Link } from "@tiptap/extension-link"
import { Placeholder } from "@tiptap/extension-placeholder"

interface TiptapMarkdownEditorProps {
  content: string
  onChange: (markdown: string) => void
  onSelectionChange?: (info: {
    text: string
    startLine: number
    endLine: number
    from: number
    to: number
    editorElement: HTMLElement
  } | null) => void
  placeholder?: string
  "data-testid"?: string
}

export function TiptapMarkdownEditor({
  content,
  onChange,
  onSelectionChange,
  placeholder = "Start writing...",
  "data-testid": testId,
}: TiptapMarkdownEditorProps) {
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange
  const onSelectionChangeRef = useRef(onSelectionChange)
  onSelectionChangeRef.current = onSelectionChange
  const lastContentRef = useRef(content)
  const suppressUpdateRef = useRef(false)

  const editor = useEditor({
    immediatelyRender: false,
    extensions: [
      StarterKit.configure({
        codeBlock: {
          HTMLAttributes: {
            class: "tiptap-code-block",
          },
        },
      }),
      Markdown,
      Table.configure({ resizable: false }),
      TableRow,
      TableCell,
      TableHeader,
      TaskList,
      TaskItem.configure({ nested: true }),
      Link.configure({
        openOnClick: false,
        autolink: true,
        HTMLAttributes: {
          class: "tiptap-link",
        },
      }),
      Placeholder.configure({
        placeholder,
      }),
    ],
    content,
    contentType: "markdown",
    onUpdate: ({ editor: ed }) => {
      if (suppressUpdateRef.current) return
      const md = ed.getMarkdown()
      lastContentRef.current = md
      onChangeRef.current(md)
    },
    onSelectionUpdate: ({ editor: ed }) => {
      const { from, to } = ed.state.selection
      if (from === to) {
        onSelectionChangeRef.current?.(null)
        return
      }
      const text = ed.state.doc.textBetween(from, to, "\n")
      if (text.trim().length === 0) {
        onSelectionChangeRef.current?.(null)
        return
      }

      let startLine = 1
      let endLine = 1
      let lineCount = 0
      ed.state.doc.descendants((node, pos) => {
        if (node.isBlock) {
          lineCount++
          if (pos <= from) startLine = lineCount
          if (pos <= to) endLine = lineCount
        }
        return true
      })

      const el = ed.view.dom
      onSelectionChangeRef.current?.({
        text: text.length > 1200 ? `${text.slice(0, 1200)}\n...[truncated]` : text,
        startLine,
        endLine,
        from,
        to,
        editorElement: el,
      })
    },
  })

  useEffect(() => {
    if (!editor || editor.isDestroyed) return
    if (content === lastContentRef.current) return
    lastContentRef.current = content

    suppressUpdateRef.current = true
    const { from, to } = editor.state.selection
    editor.commands.setContent(content, { contentType: "markdown", emitUpdate: false })
    try {
      const maxPos = editor.state.doc.content.size
      const safeFrom = Math.min(from, maxPos)
      const safeTo = Math.min(to, maxPos)
      editor.commands.setTextSelection({ from: safeFrom, to: safeTo })
    } catch {
      // ignore selection restore errors
    }
    suppressUpdateRef.current = false
  }, [content, editor])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault()
      }
    },
    [],
  )

  return (
    <div
      className="tiptap-editor-wrapper"
      data-testid={testId}
      onKeyDown={handleKeyDown}
    >
      <EditorContent editor={editor} />
    </div>
  )
}
