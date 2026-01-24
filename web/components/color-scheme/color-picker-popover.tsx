"use client"

import { useState, useRef, useEffect, useCallback } from "react"
import { Pencil } from "lucide-react"
import { cn } from "@/lib/utils"

interface ColorPickerPopoverProps {
  color: string
  label: string
  onChange: (color: string) => void
  isHighlighted: boolean
  onHover: (highlighted: boolean) => void
}

function hexToHsv(hex: string): { h: number; s: number; v: number } {
  const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex)
  if (!result) return { h: 0, s: 0, v: 100 }

  const r = parseInt(result[1], 16) / 255
  const g = parseInt(result[2], 16) / 255
  const b = parseInt(result[3], 16) / 255

  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const d = max - min

  let h = 0
  if (d !== 0) {
    switch (max) {
      case r:
        h = ((g - b) / d + (g < b ? 6 : 0)) / 6
        break
      case g:
        h = ((b - r) / d + 2) / 6
        break
      case b:
        h = ((r - g) / d + 4) / 6
        break
    }
  }

  const s = max === 0 ? 0 : d / max
  const v = max

  return { h: h * 360, s: s * 100, v: v * 100 }
}

function hsvToHex(h: number, s: number, v: number): string {
  const hNorm = h / 360
  const sNorm = s / 100
  const vNorm = v / 100

  const i = Math.floor(hNorm * 6)
  const f = hNorm * 6 - i
  const p = vNorm * (1 - sNorm)
  const q = vNorm * (1 - f * sNorm)
  const t = vNorm * (1 - (1 - f) * sNorm)

  let r = 0, g = 0, b = 0
  switch (i % 6) {
    case 0: r = vNorm; g = t; b = p; break
    case 1: r = q; g = vNorm; b = p; break
    case 2: r = p; g = vNorm; b = t; break
    case 3: r = p; g = q; b = vNorm; break
    case 4: r = t; g = p; b = vNorm; break
    case 5: r = vNorm; g = p; b = q; break
  }

  const toHex = (n: number) => Math.round(n * 255).toString(16).padStart(2, "0")
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`.toUpperCase()
}

function SaturationValuePicker({
  hue,
  saturation,
  value,
  onChange,
}: {
  hue: number
  saturation: number
  value: number
  onChange: (s: number, v: number) => void
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isDragging, setIsDragging] = useState(false)

  const updateColor = useCallback((clientX: number, clientY: number) => {
    if (!containerRef.current) return
    const rect = containerRef.current.getBoundingClientRect()
    const x = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
    const y = Math.max(0, Math.min(1, (clientY - rect.top) / rect.height))
    onChange(x * 100, (1 - y) * 100)
  }, [onChange])

  useEffect(() => {
    if (!isDragging) return

    const handleMove = (e: MouseEvent) => {
      e.preventDefault()
      updateColor(e.clientX, e.clientY)
    }
    const handleUp = () => setIsDragging(false)

    window.addEventListener("mousemove", handleMove)
    window.addEventListener("mouseup", handleUp)
    return () => {
      window.removeEventListener("mousemove", handleMove)
      window.removeEventListener("mouseup", handleUp)
    }
  }, [isDragging, updateColor])

  const hueColor = hsvToHex(hue, 100, 100)

  return (
    <div
      ref={containerRef}
      className="relative w-full h-36 rounded-lg cursor-crosshair overflow-hidden"
      style={{
        background: `
          linear-gradient(to top, #000, transparent),
          linear-gradient(to right, #fff, ${hueColor})
        `,
      }}
      onMouseDown={(e) => {
        setIsDragging(true)
        updateColor(e.clientX, e.clientY)
      }}
    >
      {/* Picker handle */}
      <div
        className="absolute w-4 h-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-md pointer-events-none"
        style={{
          left: `${saturation}%`,
          top: `${100 - value}%`,
          backgroundColor: hsvToHex(hue, saturation, value),
        }}
      />
    </div>
  )
}

function HueSlider({
  hue,
  onChange,
}: {
  hue: number
  onChange: (h: number) => void
}) {
  const containerRef = useRef<HTMLDivElement>(null)
  const [isDragging, setIsDragging] = useState(false)

  const updateHue = useCallback((clientX: number) => {
    if (!containerRef.current) return
    const rect = containerRef.current.getBoundingClientRect()
    const x = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))
    onChange(x * 360)
  }, [onChange])

  useEffect(() => {
    if (!isDragging) return

    const handleMove = (e: MouseEvent) => {
      e.preventDefault()
      updateHue(e.clientX)
    }
    const handleUp = () => setIsDragging(false)

    window.addEventListener("mousemove", handleMove)
    window.addEventListener("mouseup", handleUp)
    return () => {
      window.removeEventListener("mousemove", handleMove)
      window.removeEventListener("mouseup", handleUp)
    }
  }, [isDragging, updateHue])

  return (
    <div
      ref={containerRef}
      className="relative w-full h-3 rounded-full cursor-pointer"
      style={{
        background: "linear-gradient(to right, #f00 0%, #ff0 17%, #0f0 33%, #0ff 50%, #00f 67%, #f0f 83%, #f00 100%)",
      }}
      onMouseDown={(e) => {
        setIsDragging(true)
        updateHue(e.clientX)
      }}
    >
      {/* Slider handle */}
      <div
        className="absolute top-1/2 w-4 h-4 -translate-x-1/2 -translate-y-1/2 rounded-full border-2 border-white shadow-md pointer-events-none"
        style={{
          left: `${(hue / 360) * 100}%`,
          backgroundColor: hsvToHex(hue, 100, 100),
        }}
      />
    </div>
  )
}

export function ColorPickerPopover({
  color,
  label,
  onChange,
  isHighlighted,
  onHover,
}: ColorPickerPopoverProps) {
  const [isOpen, setIsOpen] = useState(false)
  const [hsv, setHsv] = useState(() => hexToHsv(color))
  const [hexInput, setHexInput] = useState(color.toUpperCase())
  const popoverRef = useRef<HTMLDivElement>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)

  // Sync when color prop changes externally
  useEffect(() => {
    setHsv(hexToHsv(color))
    setHexInput(color.toUpperCase())
  }, [color])

  // Close on outside click
  useEffect(() => {
    if (!isOpen) return
    const handleClick = (e: MouseEvent) => {
      if (
        popoverRef.current &&
        !popoverRef.current.contains(e.target as Node) &&
        buttonRef.current &&
        !buttonRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false)
      }
    }
    document.addEventListener("mousedown", handleClick)
    return () => document.removeEventListener("mousedown", handleClick)
  }, [isOpen])

  const handleHsvChange = useCallback((newHsv: { h?: number; s?: number; v?: number }) => {
    const updated = { ...hsv, ...newHsv }
    setHsv(updated)
    const newHex = hsvToHex(updated.h, updated.s, updated.v)
    setHexInput(newHex)
    onChange(newHex)
  }, [hsv, onChange])

  const handleHexInputChange = (value: string) => {
    // Allow typing with or without #
    let cleaned = value.replace(/[^a-fA-F0-9#]/g, "")
    if (!cleaned.startsWith("#")) {
      cleaned = "#" + cleaned
    }
    cleaned = cleaned.slice(0, 7)
    setHexInput(cleaned.toUpperCase())

    // Only update if valid hex
    if (/^#[a-fA-F0-9]{6}$/.test(cleaned)) {
      setHsv(hexToHsv(cleaned))
      onChange(cleaned.toUpperCase())
    }
  }

  const currentHex = hsvToHex(hsv.h, hsv.s, hsv.v)

  return (
    <div className="relative">
      <button
        ref={buttonRef}
        className="group flex items-center gap-1.5 cursor-pointer"
        onClick={() => setIsOpen(!isOpen)}
        onMouseEnter={() => onHover(true)}
        onMouseLeave={() => onHover(false)}
      >
        <div
          className={cn(
            "w-4 h-4 rounded-full border-2 transition-all duration-200",
            isHighlighted || isOpen
              ? "scale-125 border-white/60 shadow-lg"
              : "border-white/20",
          )}
          style={{ backgroundColor: color }}
        />
        <span
          className={cn(
            "text-[10px] transition-all duration-200",
            isHighlighted || isOpen ? "opacity-100" : "opacity-0",
          )}
        >
          {label}
        </span>
      </button>

      {isOpen && (
        <div
          ref={popoverRef}
          className="absolute bottom-full left-1/2 -translate-x-1/2 mb-3 w-56 bg-popover border border-border rounded-xl shadow-2xl z-50 overflow-hidden animate-in fade-in-0 zoom-in-95 slide-in-from-bottom-2"
        >
          {/* Header */}
          <div className="px-3 py-2.5 border-b border-border/50 flex items-center justify-between">
            <span className="text-xs font-medium">{label}</span>
            <div
              className="w-5 h-5 rounded-md border border-border/50"
              style={{ backgroundColor: currentHex }}
            />
          </div>

          {/* Color picker area */}
          <div className="p-3 space-y-3">
            <SaturationValuePicker
              hue={hsv.h}
              saturation={hsv.s}
              value={hsv.v}
              onChange={(s, v) => handleHsvChange({ s, v })}
            />

            <HueSlider
              hue={hsv.h}
              onChange={(h) => handleHsvChange({ h })}
            />

            {/* Hex input */}
            <div className="flex items-center gap-2">
              <div
                className="w-8 h-8 rounded-lg border border-border/50 flex-shrink-0"
                style={{ backgroundColor: currentHex }}
              />
              <div className="flex-1 relative">
                <input
                  type="text"
                  value={hexInput}
                  onChange={(e) => handleHexInputChange(e.target.value)}
                  className="w-full h-8 px-2.5 pr-8 rounded-lg bg-muted/50 border border-border/50 text-xs font-mono uppercase focus:outline-none focus:ring-2 focus:ring-primary/30"
                  placeholder="#000000"
                  maxLength={7}
                />
                <Pencil className="absolute right-2.5 top-1/2 -translate-y-1/2 w-3 h-3 text-muted-foreground" />
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
