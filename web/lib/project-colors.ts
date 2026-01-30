"use client"

const PROJECT_COLOR_CLASSES = [
  "bg-violet-500",
  "bg-emerald-500",
  "bg-blue-500",
  "bg-amber-500",
  "bg-rose-500",
  "bg-cyan-500",
] as const

function hashString(input: string): number {
  let h = 2166136261
  for (let i = 0; i < input.length; i += 1) {
    h ^= input.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return h >>> 0
}

export function projectColorClass(projectId: string): string {
  const idx = hashString(projectId) % PROJECT_COLOR_CLASSES.length
  return PROJECT_COLOR_CLASSES[idx]!
}

