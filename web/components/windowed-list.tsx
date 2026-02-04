"use client"

import type React from "react"

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"

import { cn } from "@/lib/utils"

export type WindowedListItem = {
  key: string
  node: React.ReactNode
}

function findStartIndex(offsets: number[], target: number): number {
  let lo = 0
  let hi = offsets.length - 1
  while (lo < hi) {
    const mid = Math.floor((lo + hi) / 2)
    if (offsets[mid]! < target) lo = mid + 1
    else hi = mid
  }
  return lo
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value))
}

function scrollTopForElement(el: HTMLElement, child: HTMLElement): number {
  const elRect = el.getBoundingClientRect()
  const childRect = child.getBoundingClientRect()
  return childRect.top - elRect.top + el.scrollTop
}

export function WindowedList({
  items,
  listKey,
  scrollElement,
  emptyState,
  className,
  itemClassName,
  estimatedItemHeightPx = 140,
  overscanPx = 800,
}: {
  items: WindowedListItem[]
  listKey: string
  scrollElement: HTMLElement | null
  emptyState?: React.ReactNode
  className?: string
  itemClassName?: string
  estimatedItemHeightPx?: number
  overscanPx?: number
}): React.ReactElement | null {
  const rootRef = useRef<HTMLDivElement | null>(null)
  const scrollElementRef = useRef<HTMLElement | null>(null)
  scrollElementRef.current = scrollElement

  const sizesRef = useRef<Map<string, number>>(new Map())
  const rowNodesRef = useRef<Map<string, HTMLElement>>(new Map())
  const rowResizeObserverRef = useRef<ResizeObserver | null>(null)
  const [sizesVersion, setSizesVersion] = useState(0)

  const prevListKeyRef = useRef<string>(listKey)
  const prevItemKeysRef = useRef<string[]>([])
  const prevTotalHeightRef = useRef(0)

  const [viewport, setViewport] = useState({ scrollTop: 0, height: 0, listTop: 0 })
  const viewportRef = useRef(viewport)
  viewportRef.current = viewport

  const indexByKey = useMemo(() => {
    return new Map(items.map((it, idx) => [it.key, idx]))
  }, [items])
  const indexByKeyRef = useRef(indexByKey)
  indexByKeyRef.current = indexByKey

  const pendingScrollRef = useRef<number | null>(null)

  useLayoutEffect(() => {
    if (prevListKeyRef.current !== listKey) {
      prevListKeyRef.current = listKey
      prevItemKeysRef.current = []
      prevTotalHeightRef.current = 0
      rowNodesRef.current = new Map()
      sizesRef.current = new Map()
      setSizesVersion((v) => v + 1)
    }
  }, [listKey])

  useEffect(() => {
    const el = scrollElement
    if (!el) return

    const update = () => {
      pendingScrollRef.current = null
      const listTop =
        rootRef.current != null ? scrollTopForElement(el, rootRef.current) : 0
      setViewport({ scrollTop: el.scrollTop, height: el.clientHeight, listTop })
    }

    const onScroll = () => {
      if (pendingScrollRef.current != null) return
      pendingScrollRef.current = window.requestAnimationFrame(update)
    }

    const resizeObserver = new ResizeObserver(() => update())
    resizeObserver.observe(el)

    update()
    el.addEventListener("scroll", onScroll, { passive: true })
    return () => {
      if (pendingScrollRef.current != null) {
        window.cancelAnimationFrame(pendingScrollRef.current)
        pendingScrollRef.current = null
      }
      resizeObserver.disconnect()
      el.removeEventListener("scroll", onScroll)
    }
  }, [scrollElement])

  const layout = useMemo(() => {
    void sizesVersion
    const offsets: number[] = new Array(items.length + 1)
    offsets[0] = 0
    for (let i = 0; i < items.length; i += 1) {
      const key = items[i]?.key ?? String(i)
      const size = sizesRef.current.get(key) ?? estimatedItemHeightPx
      offsets[i + 1] = offsets[i]! + size
    }
    const totalHeight = offsets[items.length] ?? 0
    return { offsets, totalHeight }
  }, [estimatedItemHeightPx, items, sizesVersion])

  const offsetsRef = useRef<number[]>(layout.offsets)
  offsetsRef.current = layout.offsets

  useLayoutEffect(() => {
    const el = scrollElementRef.current
    if (!el) return

    const prevKeys = prevItemKeysRef.current
    const nextKeys = items.map((it) => it.key)

    if (prevKeys.length > 0 && nextKeys.length > prevKeys.length && prevKeys[0] !== nextKeys[0]) {
      const pivot = nextKeys.indexOf(prevKeys[0]!)
      const looksLikePurePrepend =
        pivot > 0 &&
        nextKeys.length - pivot === prevKeys.length &&
        prevKeys[prevKeys.length - 1] === nextKeys[nextKeys.length - 1]

      if (looksLikePurePrepend) {
        const delta = layout.totalHeight - prevTotalHeightRef.current
        if (delta !== 0) el.scrollTop = el.scrollTop + delta
      }
    }

    prevItemKeysRef.current = nextKeys
    prevTotalHeightRef.current = layout.totalHeight
  }, [items, layout.totalHeight, scrollElement])

  useEffect(() => {
    prevTotalHeightRef.current = layout.totalHeight
  }, [layout.totalHeight])

  useEffect(() => {
    const el = scrollElementRef.current
    if (!el) return

    const observer = new ResizeObserver((entries) => {
      const snap = viewportRef.current
      const offsets = offsetsRef.current
      const indexMap = indexByKeyRef.current
      const initialScrollTop = el.scrollTop

      let changed = false
      let scrollAdjustment = 0

      for (const entry of entries) {
        const node = entry.target as HTMLElement
        const key = node.dataset.windowedKey
        if (!key) continue

        const nextHeight = Math.ceil(node.offsetHeight)
        const prevHeight = sizesRef.current.get(key) ?? estimatedItemHeightPx
        if (nextHeight === prevHeight) continue

        changed = true
        sizesRef.current.set(key, nextHeight)

        const idx = indexMap.get(key)
        if (idx == null) continue

        const itemTopInList = offsets[idx] ?? 0
        const itemBottomInScroll = snap.listTop + itemTopInList + prevHeight
        if (itemBottomInScroll <= initialScrollTop) {
          scrollAdjustment += nextHeight - prevHeight
        }
      }

      if (!changed) return
      if (scrollAdjustment !== 0) {
        el.scrollTop = initialScrollTop + scrollAdjustment
      }
      setSizesVersion((v) => v + 1)
    })

    rowResizeObserverRef.current = observer
    for (const node of rowNodesRef.current.values()) observer.observe(node)
    return () => {
      rowResizeObserverRef.current = null
      observer.disconnect()
    }
  }, [estimatedItemHeightPx, listKey, scrollElement])

  const range = useMemo(() => {
    if (items.length === 0) return { start: 0, end: 0 }

    const virtualScrollTop = Math.max(0, viewport.scrollTop - viewport.listTop)
    const startPx = Math.max(0, virtualScrollTop - overscanPx)
    const endPx = virtualScrollTop + viewport.height + overscanPx

    const start = clamp(findStartIndex(layout.offsets, startPx) - 1, 0, items.length)
    const end = clamp(findStartIndex(layout.offsets, endPx) + 1, start, items.length)
    return { start, end }
  }, [items.length, layout.offsets, overscanPx, viewport.height, viewport.listTop, viewport.scrollTop])

  if (items.length === 0) {
    return emptyState ? <>{emptyState}</> : null
  }

  return (
    <div ref={rootRef} className={cn(className)}>
      <div className="relative" style={{ height: layout.totalHeight }}>
        {items.slice(range.start, range.end).map((item, idx) => {
          const i = range.start + idx
          const key = item.key
          const top = layout.offsets[i] ?? 0
          return (
            <div
              key={key}
              data-windowed-key={key}
              className={cn("absolute left-0 right-0", itemClassName)}
              style={{ transform: `translateY(${top}px)` }}
              ref={(node) => {
                const observer = rowResizeObserverRef.current
                const prev = rowNodesRef.current.get(key) ?? null
                if (prev && (!node || node !== prev)) {
                  if (observer) observer.unobserve(prev)
                  rowNodesRef.current.delete(key)
                }
                if (node) {
                  rowNodesRef.current.set(key, node)
                  if (observer) observer.observe(node)
                  const height = Math.ceil(node.offsetHeight)
                  const current = sizesRef.current.get(key)
                  if (current !== height) {
                    sizesRef.current.set(key, height)
                    setSizesVersion((v) => v + 1)
                  }
                }
              }}
            >
              {item.node}
            </div>
          )
        })}
      </div>
    </div>
  )
}
