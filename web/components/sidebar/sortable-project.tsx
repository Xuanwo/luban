"use client"

import type React from "react"
import { useSortable } from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { cn } from "@/lib/utils"

interface SortableProjectProps {
  id: string
  children: React.ReactNode
  disabled?: boolean
}

export function SortableProject({ id, children, disabled }: SortableProjectProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({
    id,
    disabled,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    zIndex: isDragging ? 50 : undefined,
    position: "relative" as const,
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn("group/project", isDragging && "opacity-0")}
      {...attributes}
      {...listeners}
    >
      {children}
    </div>
  )
}
