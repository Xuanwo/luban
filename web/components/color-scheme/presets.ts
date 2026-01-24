import type { ColorScheme } from "./types"

// Default Light Theme (current theme)
export const defaultLight: ColorScheme = {
  id: "default",
  name: "Default",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(1 0 0)",
    foreground: "oklch(0.145 0 0)",
    card: "oklch(1 0 0)",
    cardForeground: "oklch(0.145 0 0)",
    popover: "oklch(1 0 0)",
    popoverForeground: "oklch(0.145 0 0)",
    primary: "oklch(0.205 0 0)",
    primaryForeground: "oklch(0.985 0 0)",
    secondary: "oklch(0.97 0 0)",
    secondaryForeground: "oklch(0.205 0 0)",
    muted: "oklch(0.97 0 0)",
    mutedForeground: "oklch(0.556 0 0)",
    accent: "oklch(0.97 0 0)",
    accentForeground: "oklch(0.205 0 0)",
    destructive: "oklch(0.577 0.245 27.325)",
    destructiveForeground: "oklch(0.577 0.245 27.325)",
    border: "oklch(0.922 0 0)",
    input: "oklch(0.922 0 0)",
    ring: "oklch(0.708 0 0)",
    sidebar: "oklch(0.985 0 0)",
    sidebarForeground: "oklch(0.145 0 0)",
    sidebarPrimary: "oklch(0.205 0 0)",
    sidebarPrimaryForeground: "oklch(0.985 0 0)",
    sidebarAccent: "oklch(0.97 0 0)",
    sidebarAccentForeground: "oklch(0.205 0 0)",
    sidebarBorder: "oklch(0.922 0 0)",
    sidebarRing: "oklch(0.708 0 0)",
    chart1: "oklch(0.646 0.222 41.116)",
    chart2: "oklch(0.6 0.118 184.704)",
    chart3: "oklch(0.398 0.07 227.392)",
    chart4: "oklch(0.828 0.189 84.429)",
    chart5: "oklch(0.769 0.188 70.08)",
  },
  // Order: background, primary, success, error, accent, foreground
  previewColors: ["#ffffff", "#3b82f6", "#22c55e", "#ef4444", "#8b5cf6", "#1a1a1a"],
}

// Default Dark Theme (current theme)
export const defaultDark: ColorScheme = {
  id: "default",
  name: "Default",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.145 0 0)",
    foreground: "oklch(0.985 0 0)",
    card: "oklch(0.145 0 0)",
    cardForeground: "oklch(0.985 0 0)",
    popover: "oklch(0.145 0 0)",
    popoverForeground: "oklch(0.985 0 0)",
    primary: "oklch(0.985 0 0)",
    primaryForeground: "oklch(0.205 0 0)",
    secondary: "oklch(0.269 0 0)",
    secondaryForeground: "oklch(0.985 0 0)",
    muted: "oklch(0.269 0 0)",
    mutedForeground: "oklch(0.708 0 0)",
    accent: "oklch(0.269 0 0)",
    accentForeground: "oklch(0.985 0 0)",
    destructive: "oklch(0.396 0.141 25.723)",
    destructiveForeground: "oklch(0.637 0.237 25.331)",
    border: "oklch(0.269 0 0)",
    input: "oklch(0.269 0 0)",
    ring: "oklch(0.439 0 0)",
    sidebar: "oklch(0.205 0 0)",
    sidebarForeground: "oklch(0.985 0 0)",
    sidebarPrimary: "oklch(0.488 0.243 264.376)",
    sidebarPrimaryForeground: "oklch(0.985 0 0)",
    sidebarAccent: "oklch(0.269 0 0)",
    sidebarAccentForeground: "oklch(0.985 0 0)",
    sidebarBorder: "oklch(0.269 0 0)",
    sidebarRing: "oklch(0.439 0 0)",
    chart1: "oklch(0.488 0.243 264.376)",
    chart2: "oklch(0.696 0.17 162.48)",
    chart3: "oklch(0.769 0.188 70.08)",
    chart4: "oklch(0.627 0.265 303.9)",
    chart5: "oklch(0.645 0.246 16.439)",
  },
  // Order: background, primary, success, error, accent, foreground
  previewColors: ["#1a1a1a", "#3b82f6", "#22c55e", "#ef4444", "#a78bfa", "#fafafa"],
}

// Flexoki Light Theme
// Based on https://github.com/kepano/flexoki
export const flexokiLight: ColorScheme = {
  id: "flexoki-light",
  name: "Flexoki Light",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.988 0.007 90)", // #FFFCF0 paper
    foreground: "oklch(0.13 0.02 75)", // #100F0F black
    card: "oklch(0.988 0.007 90)", // same as background for unified look
    cardForeground: "oklch(0.13 0.02 75)", // #100F0F
    popover: "oklch(0.988 0.007 90)",
    popoverForeground: "oklch(0.13 0.02 75)",
    primary: "oklch(0.47 0.13 250)", // #205EA6 blue-600
    primaryForeground: "oklch(0.988 0.007 90)",
    secondary: "oklch(0.92 0.01 90)", // #E6E4D9 base-100
    secondaryForeground: "oklch(0.13 0.02 75)",
    muted: "oklch(0.88 0.01 90)", // #DAD8CE base-150
    mutedForeground: "oklch(0.5 0.02 75)", // #6F6E69 base-600
    accent: "oklch(0.92 0.01 90)",
    accentForeground: "oklch(0.13 0.02 75)",
    destructive: "oklch(0.5 0.17 25)", // #AF3029 red-600
    destructiveForeground: "oklch(0.988 0.007 90)",
    border: "oklch(0.84 0.01 90)", // #CECDC3 base-200
    input: "oklch(0.84 0.01 90)",
    ring: "oklch(0.47 0.13 250)",
    sidebar: "oklch(0.955 0.008 90)",
    sidebarForeground: "oklch(0.13 0.02 75)",
    sidebarPrimary: "oklch(0.47 0.13 250)",
    sidebarPrimaryForeground: "oklch(0.988 0.007 90)",
    sidebarAccent: "oklch(0.92 0.01 90)",
    sidebarAccentForeground: "oklch(0.13 0.02 75)",
    sidebarBorder: "oklch(0.84 0.01 90)",
    sidebarRing: "oklch(0.47 0.13 250)",
    chart1: "oklch(0.63 0.19 25)", // #D14D41 red-400
    chart2: "oklch(0.6 0.17 50)", // #DA702C orange-400
    chart3: "oklch(0.65 0.14 115)", // #879A39 green-400
    chart4: "oklch(0.6 0.12 240)", // #4385BE blue-400
    chart5: "oklch(0.6 0.12 290)", // #8B7EC8 purple-400
  },
  previewColors: ["#FFFCF0", "#4385BE", "#879A39", "#D14D41", "#8B7EC8", "#100F0F"],
}

// Flexoki Dark Theme
export const flexokiDark: ColorScheme = {
  id: "flexoki-dark",
  name: "Flexoki Dark",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.13 0.02 75)", // #100F0F black
    foreground: "oklch(0.84 0.01 90)", // #CECDC3 base-200
    card: "oklch(0.13 0.02 75)", // same as background for unified look
    cardForeground: "oklch(0.84 0.01 90)",
    popover: "oklch(0.13 0.02 75)",
    popoverForeground: "oklch(0.84 0.01 90)",
    primary: "oklch(0.6 0.12 240)", // #4385BE blue-400
    primaryForeground: "oklch(0.13 0.02 75)",
    secondary: "oklch(0.25 0.015 75)", // #282726 base-900
    secondaryForeground: "oklch(0.84 0.01 90)",
    muted: "oklch(0.3 0.015 75)", // #343331 base-850
    mutedForeground: "oklch(0.6 0.01 90)", // #878580 base-500
    accent: "oklch(0.25 0.015 75)",
    accentForeground: "oklch(0.84 0.01 90)",
    destructive: "oklch(0.63 0.19 25)", // #D14D41 red-400
    destructiveForeground: "oklch(0.13 0.02 75)",
    border: "oklch(0.35 0.015 75)", // #403E3C base-800
    input: "oklch(0.35 0.015 75)",
    ring: "oklch(0.6 0.12 240)",
    sidebar: "oklch(0.2 0.015 75)",
    sidebarForeground: "oklch(0.84 0.01 90)",
    sidebarPrimary: "oklch(0.6 0.12 240)",
    sidebarPrimaryForeground: "oklch(0.84 0.01 90)",
    sidebarAccent: "oklch(0.25 0.015 75)",
    sidebarAccentForeground: "oklch(0.84 0.01 90)",
    sidebarBorder: "oklch(0.35 0.015 75)",
    sidebarRing: "oklch(0.6 0.12 240)",
    chart1: "oklch(0.63 0.19 25)", // #D14D41 red-400
    chart2: "oklch(0.6 0.17 50)", // #DA702C orange-400
    chart3: "oklch(0.65 0.14 115)", // #879A39 green-400
    chart4: "oklch(0.6 0.12 240)", // #4385BE blue-400
    chart5: "oklch(0.6 0.12 290)", // #8B7EC8 purple-400
  },
  previewColors: ["#100F0F", "#4385BE", "#879A39", "#D14D41", "#8B7EC8", "#CECDC3"],
}

// Catppuccin Latte (Light)
export const catppuccinLatte: ColorScheme = {
  id: "catppuccin-latte",
  name: "Catppuccin Latte",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.965 0.015 240)", // #EFF1F5 base
    foreground: "oklch(0.35 0.05 265)", // #4C4F69 text
    card: "oklch(0.965 0.015 240)", // same as background for unified look
    cardForeground: "oklch(0.35 0.05 265)",
    popover: "oklch(0.965 0.015 240)",
    popoverForeground: "oklch(0.35 0.05 265)",
    primary: "oklch(0.55 0.2 270)", // #7287FD lavender
    primaryForeground: "oklch(0.965 0.015 240)",
    secondary: "oklch(0.88 0.02 250)", // #CCD0DA surface0
    secondaryForeground: "oklch(0.35 0.05 265)",
    muted: "oklch(0.85 0.02 250)", // #BCC0CC surface1
    mutedForeground: "oklch(0.5 0.04 265)", // #6C6F85 subtext0
    accent: "oklch(0.65 0.15 180)", // #179299 teal
    accentForeground: "oklch(0.965 0.015 240)",
    destructive: "oklch(0.6 0.2 15)", // #D20F39 red
    destructiveForeground: "oklch(0.965 0.015 240)",
    border: "oklch(0.88 0.02 250)",
    input: "oklch(0.88 0.02 250)",
    ring: "oklch(0.55 0.2 270)",
    sidebar: "oklch(0.94 0.015 245)",
    sidebarForeground: "oklch(0.35 0.05 265)",
    sidebarPrimary: "oklch(0.55 0.2 270)",
    sidebarPrimaryForeground: "oklch(0.965 0.015 240)",
    sidebarAccent: "oklch(0.88 0.02 250)",
    sidebarAccentForeground: "oklch(0.35 0.05 265)",
    sidebarBorder: "oklch(0.88 0.02 250)",
    sidebarRing: "oklch(0.55 0.2 270)",
    chart1: "oklch(0.6 0.2 15)", // red
    chart2: "oklch(0.65 0.18 75)", // peach
    chart3: "oklch(0.7 0.18 130)", // green
    chart4: "oklch(0.6 0.18 220)", // blue
    chart5: "oklch(0.55 0.2 270)", // lavender
  },
  previewColors: ["#EFF1F5", "#7287FD", "#40A02B", "#D20F39", "#8839EF", "#4C4F69"],
}

// Catppuccin Mocha (Dark)
export const catppuccinMocha: ColorScheme = {
  id: "catppuccin-mocha",
  name: "Catppuccin Mocha",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.25 0.015 265)", // #1E1E2E base
    foreground: "oklch(0.9 0.01 260)", // #CDD6F4 text
    card: "oklch(0.25 0.015 265)", // same as background for unified look
    cardForeground: "oklch(0.9 0.01 260)",
    popover: "oklch(0.25 0.015 265)",
    popoverForeground: "oklch(0.9 0.01 260)",
    primary: "oklch(0.72 0.15 270)", // #B4BEFE lavender
    primaryForeground: "oklch(0.25 0.015 265)",
    secondary: "oklch(0.35 0.02 265)", // #313244 surface0
    secondaryForeground: "oklch(0.9 0.01 260)",
    muted: "oklch(0.4 0.02 265)", // #45475A surface1
    mutedForeground: "oklch(0.65 0.02 260)", // #A6ADC8 subtext0
    accent: "oklch(0.75 0.12 180)", // #94E2D5 teal
    accentForeground: "oklch(0.25 0.015 265)",
    destructive: "oklch(0.7 0.2 15)", // #F38BA8 red
    destructiveForeground: "oklch(0.25 0.015 265)",
    border: "oklch(0.35 0.02 265)",
    input: "oklch(0.35 0.02 265)",
    ring: "oklch(0.72 0.15 270)",
    sidebar: "oklch(0.22 0.015 265)",
    sidebarForeground: "oklch(0.9 0.01 260)",
    sidebarPrimary: "oklch(0.72 0.15 270)",
    sidebarPrimaryForeground: "oklch(0.25 0.015 265)",
    sidebarAccent: "oklch(0.35 0.02 265)",
    sidebarAccentForeground: "oklch(0.9 0.01 260)",
    sidebarBorder: "oklch(0.35 0.02 265)",
    sidebarRing: "oklch(0.72 0.15 270)",
    chart1: "oklch(0.7 0.2 15)", // red
    chart2: "oklch(0.75 0.15 75)", // peach
    chart3: "oklch(0.8 0.15 130)", // green
    chart4: "oklch(0.72 0.15 220)", // blue
    chart5: "oklch(0.72 0.15 270)", // lavender
  },
  previewColors: ["#1E1E2E", "#B4BEFE", "#A6E3A1", "#F38BA8", "#CBA6F7", "#CDD6F4"],
}

// Solarized Light
export const solarizedLight: ColorScheme = {
  id: "solarized-light",
  name: "Solarized Light",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.98 0.02 95)", // #FDF6E3 base3
    foreground: "oklch(0.45 0.05 195)", // #657B83 base00
    card: "oklch(0.98 0.02 95)", // same as background for unified look
    cardForeground: "oklch(0.45 0.05 195)",
    popover: "oklch(0.98 0.02 95)",
    popoverForeground: "oklch(0.45 0.05 195)",
    primary: "oklch(0.5 0.15 230)", // #268BD2 blue
    primaryForeground: "oklch(0.98 0.02 95)",
    secondary: "oklch(0.92 0.02 95)", // #EEE8D5
    secondaryForeground: "oklch(0.45 0.05 195)",
    muted: "oklch(0.88 0.02 95)",
    mutedForeground: "oklch(0.55 0.04 195)", // base01
    accent: "oklch(0.6 0.15 180)", // #2AA198 cyan
    accentForeground: "oklch(0.98 0.02 95)",
    destructive: "oklch(0.55 0.2 25)", // #DC322F red
    destructiveForeground: "oklch(0.98 0.02 95)",
    border: "oklch(0.85 0.02 95)",
    input: "oklch(0.85 0.02 95)",
    ring: "oklch(0.5 0.15 230)",
    sidebar: "oklch(0.95 0.02 95)",
    sidebarForeground: "oklch(0.45 0.05 195)",
    sidebarPrimary: "oklch(0.5 0.15 230)",
    sidebarPrimaryForeground: "oklch(0.98 0.02 95)",
    sidebarAccent: "oklch(0.92 0.02 95)",
    sidebarAccentForeground: "oklch(0.45 0.05 195)",
    sidebarBorder: "oklch(0.85 0.02 95)",
    sidebarRing: "oklch(0.5 0.15 230)",
    chart1: "oklch(0.55 0.2 25)", // red
    chart2: "oklch(0.6 0.17 60)", // orange
    chart3: "oklch(0.65 0.15 115)", // green
    chart4: "oklch(0.5 0.15 230)", // blue
    chart5: "oklch(0.55 0.15 290)", // violet
  },
  previewColors: ["#FDF6E3", "#268BD2", "#859900", "#DC322F", "#6C71C4", "#657B83"],
}

// Solarized Dark
export const solarizedDark: ColorScheme = {
  id: "solarized-dark",
  name: "Solarized Dark",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.2 0.03 210)", // #002B36 base03
    foreground: "oklch(0.75 0.03 95)", // #839496 base0
    card: "oklch(0.2 0.03 210)", // same as background for unified look
    cardForeground: "oklch(0.75 0.03 95)",
    popover: "oklch(0.2 0.03 210)",
    popoverForeground: "oklch(0.75 0.03 95)",
    primary: "oklch(0.6 0.15 230)", // #268BD2 blue
    primaryForeground: "oklch(0.2 0.03 210)",
    secondary: "oklch(0.3 0.03 210)",
    secondaryForeground: "oklch(0.75 0.03 95)",
    muted: "oklch(0.35 0.03 210)",
    mutedForeground: "oklch(0.6 0.03 95)", // base01
    accent: "oklch(0.7 0.12 180)", // #2AA198 cyan
    accentForeground: "oklch(0.2 0.03 210)",
    destructive: "oklch(0.6 0.2 25)", // #DC322F red
    destructiveForeground: "oklch(0.2 0.03 210)",
    border: "oklch(0.35 0.03 210)",
    input: "oklch(0.35 0.03 210)",
    ring: "oklch(0.6 0.15 230)",
    sidebar: "oklch(0.25 0.03 210)",
    sidebarForeground: "oklch(0.75 0.03 95)",
    sidebarPrimary: "oklch(0.6 0.15 230)",
    sidebarPrimaryForeground: "oklch(0.2 0.03 210)",
    sidebarAccent: "oklch(0.3 0.03 210)",
    sidebarAccentForeground: "oklch(0.75 0.03 95)",
    sidebarBorder: "oklch(0.35 0.03 210)",
    sidebarRing: "oklch(0.6 0.15 230)",
    chart1: "oklch(0.6 0.2 25)", // red
    chart2: "oklch(0.65 0.17 60)", // orange
    chart3: "oklch(0.7 0.15 115)", // green
    chart4: "oklch(0.6 0.15 230)", // blue
    chart5: "oklch(0.6 0.15 290)", // violet
  },
  previewColors: ["#002B36", "#268BD2", "#859900", "#DC322F", "#6C71C4", "#839496"],
}

// Tokyo Night Light
export const tokyoNightLight: ColorScheme = {
  id: "tokyo-night-light",
  name: "Tokyo Night Light",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.97 0.01 250)", // #D5D6DB
    foreground: "oklch(0.35 0.05 260)", // #343B58
    card: "oklch(0.97 0.01 250)", // same as background for unified look
    cardForeground: "oklch(0.35 0.05 260)",
    popover: "oklch(0.97 0.01 250)",
    popoverForeground: "oklch(0.35 0.05 260)",
    primary: "oklch(0.5 0.18 260)", // #7AA2F7
    primaryForeground: "oklch(0.97 0.01 250)",
    secondary: "oklch(0.9 0.01 250)",
    secondaryForeground: "oklch(0.35 0.05 260)",
    muted: "oklch(0.87 0.01 250)",
    mutedForeground: "oklch(0.5 0.03 260)",
    accent: "oklch(0.6 0.15 180)", // #73DACA
    accentForeground: "oklch(0.35 0.05 260)",
    destructive: "oklch(0.6 0.2 15)", // #F7768E
    destructiveForeground: "oklch(0.97 0.01 250)",
    border: "oklch(0.85 0.01 250)",
    input: "oklch(0.85 0.01 250)",
    ring: "oklch(0.5 0.18 260)",
    sidebar: "oklch(0.94 0.01 250)",
    sidebarForeground: "oklch(0.35 0.05 260)",
    sidebarPrimary: "oklch(0.5 0.18 260)",
    sidebarPrimaryForeground: "oklch(0.97 0.01 250)",
    sidebarAccent: "oklch(0.9 0.01 250)",
    sidebarAccentForeground: "oklch(0.35 0.05 260)",
    sidebarBorder: "oklch(0.85 0.01 250)",
    sidebarRing: "oklch(0.5 0.18 260)",
    chart1: "oklch(0.6 0.2 15)",
    chart2: "oklch(0.7 0.15 75)",
    chart3: "oklch(0.7 0.12 180)",
    chart4: "oklch(0.6 0.18 260)",
    chart5: "oklch(0.6 0.15 290)",
  },
  previewColors: ["#D5D6DB", "#7AA2F7", "#9ECE6A", "#F7768E", "#BB9AF7", "#343B58"],
}

// Tokyo Night Dark
export const tokyoNightDark: ColorScheme = {
  id: "tokyo-night-dark",
  name: "Tokyo Night",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.2 0.02 260)", // #1A1B26
    foreground: "oklch(0.85 0.02 260)", // #A9B1D6
    card: "oklch(0.2 0.02 260)", // same as background for unified look
    cardForeground: "oklch(0.85 0.02 260)",
    popover: "oklch(0.2 0.02 260)",
    popoverForeground: "oklch(0.85 0.02 260)",
    primary: "oklch(0.7 0.15 260)", // #7AA2F7
    primaryForeground: "oklch(0.2 0.02 260)",
    secondary: "oklch(0.28 0.02 260)",
    secondaryForeground: "oklch(0.85 0.02 260)",
    muted: "oklch(0.32 0.02 260)",
    mutedForeground: "oklch(0.6 0.02 260)",
    accent: "oklch(0.75 0.1 180)", // #73DACA
    accentForeground: "oklch(0.2 0.02 260)",
    destructive: "oklch(0.7 0.18 15)", // #F7768E
    destructiveForeground: "oklch(0.2 0.02 260)",
    border: "oklch(0.32 0.02 260)",
    input: "oklch(0.32 0.02 260)",
    ring: "oklch(0.7 0.15 260)",
    sidebar: "oklch(0.18 0.02 260)",
    sidebarForeground: "oklch(0.85 0.02 260)",
    sidebarPrimary: "oklch(0.7 0.15 260)",
    sidebarPrimaryForeground: "oklch(0.2 0.02 260)",
    sidebarAccent: "oklch(0.28 0.02 260)",
    sidebarAccentForeground: "oklch(0.85 0.02 260)",
    sidebarBorder: "oklch(0.32 0.02 260)",
    sidebarRing: "oklch(0.7 0.15 260)",
    chart1: "oklch(0.7 0.18 15)",
    chart2: "oklch(0.75 0.12 75)",
    chart3: "oklch(0.75 0.12 130)",
    chart4: "oklch(0.7 0.15 260)",
    chart5: "oklch(0.7 0.12 290)",
  },
  previewColors: ["#1A1B26", "#7AA2F7", "#9ECE6A", "#F7768E", "#BB9AF7", "#A9B1D6"],
}

// One Dark Pro
export const oneDarkPro: ColorScheme = {
  id: "one-dark-pro",
  name: "One Dark Pro",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.23 0.015 250)", // #282C34
    foreground: "oklch(0.8 0.015 250)", // #ABB2BF
    card: "oklch(0.23 0.015 250)", // same as background for unified look
    cardForeground: "oklch(0.8 0.015 250)",
    popover: "oklch(0.23 0.015 250)",
    popoverForeground: "oklch(0.8 0.015 250)",
    primary: "oklch(0.65 0.15 220)", // #61AFEF blue
    primaryForeground: "oklch(0.23 0.015 250)",
    secondary: "oklch(0.3 0.015 250)",
    secondaryForeground: "oklch(0.8 0.015 250)",
    muted: "oklch(0.35 0.015 250)",
    mutedForeground: "oklch(0.55 0.015 250)",
    accent: "oklch(0.7 0.12 170)", // #56B6C2 cyan
    accentForeground: "oklch(0.23 0.015 250)",
    destructive: "oklch(0.65 0.2 15)", // #E06C75 red
    destructiveForeground: "oklch(0.23 0.015 250)",
    border: "oklch(0.35 0.015 250)",
    input: "oklch(0.35 0.015 250)",
    ring: "oklch(0.65 0.15 220)",
    sidebar: "oklch(0.2 0.015 250)",
    sidebarForeground: "oklch(0.8 0.015 250)",
    sidebarPrimary: "oklch(0.65 0.15 220)",
    sidebarPrimaryForeground: "oklch(0.23 0.015 250)",
    sidebarAccent: "oklch(0.3 0.015 250)",
    sidebarAccentForeground: "oklch(0.8 0.015 250)",
    sidebarBorder: "oklch(0.35 0.015 250)",
    sidebarRing: "oklch(0.65 0.15 220)",
    chart1: "oklch(0.65 0.2 15)", // red
    chart2: "oklch(0.7 0.15 60)", // orange
    chart3: "oklch(0.75 0.15 115)", // green
    chart4: "oklch(0.65 0.15 220)", // blue
    chart5: "oklch(0.65 0.15 290)", // purple
  },
  previewColors: ["#282C34", "#61AFEF", "#98C379", "#E06C75", "#C678DD", "#ABB2BF"],
}

// GitHub Light
export const githubLight: ColorScheme = {
  id: "github-light",
  name: "GitHub Light",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(1 0 0)", // #FFFFFF
    foreground: "oklch(0.24 0.02 240)", // #24292F
    card: "oklch(1 0 0)", // same as background for unified look
    cardForeground: "oklch(0.24 0.02 240)",
    popover: "oklch(1 0 0)",
    popoverForeground: "oklch(0.24 0.02 240)",
    primary: "oklch(0.5 0.15 250)", // #0969DA blue
    primaryForeground: "oklch(1 0 0)",
    secondary: "oklch(0.95 0.005 240)",
    secondaryForeground: "oklch(0.24 0.02 240)",
    muted: "oklch(0.92 0.005 240)",
    mutedForeground: "oklch(0.45 0.02 240)",
    accent: "oklch(0.95 0.005 240)",
    accentForeground: "oklch(0.24 0.02 240)",
    destructive: "oklch(0.55 0.2 20)", // #CF222E red
    destructiveForeground: "oklch(1 0 0)",
    border: "oklch(0.88 0.01 240)",
    input: "oklch(0.88 0.01 240)",
    ring: "oklch(0.5 0.15 250)",
    sidebar: "oklch(0.97 0.005 240)",
    sidebarForeground: "oklch(0.24 0.02 240)",
    sidebarPrimary: "oklch(0.5 0.15 250)",
    sidebarPrimaryForeground: "oklch(1 0 0)",
    sidebarAccent: "oklch(0.95 0.005 240)",
    sidebarAccentForeground: "oklch(0.24 0.02 240)",
    sidebarBorder: "oklch(0.88 0.01 240)",
    sidebarRing: "oklch(0.5 0.15 250)",
    chart1: "oklch(0.55 0.2 20)",
    chart2: "oklch(0.6 0.15 55)",
    chart3: "oklch(0.55 0.15 140)",
    chart4: "oklch(0.5 0.15 250)",
    chart5: "oklch(0.55 0.15 290)",
  },
  previewColors: ["#FFFFFF", "#0969DA", "#1A7F37", "#CF222E", "#8250DF", "#24292F"],
}

// GitHub Dark
export const githubDark: ColorScheme = {
  id: "github-dark",
  name: "GitHub Dark",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.16 0.015 250)", // #0D1117
    foreground: "oklch(0.85 0.015 250)", // #C9D1D9
    card: "oklch(0.16 0.015 250)", // same as background for unified look
    cardForeground: "oklch(0.85 0.015 250)",
    popover: "oklch(0.16 0.015 250)",
    popoverForeground: "oklch(0.85 0.015 250)",
    primary: "oklch(0.65 0.15 230)", // #58A6FF blue
    primaryForeground: "oklch(0.16 0.015 250)",
    secondary: "oklch(0.25 0.015 250)",
    secondaryForeground: "oklch(0.85 0.015 250)",
    muted: "oklch(0.3 0.015 250)",
    mutedForeground: "oklch(0.6 0.015 250)",
    accent: "oklch(0.25 0.015 250)",
    accentForeground: "oklch(0.85 0.015 250)",
    destructive: "oklch(0.65 0.2 15)", // #F85149 red
    destructiveForeground: "oklch(0.16 0.015 250)",
    border: "oklch(0.3 0.015 250)",
    input: "oklch(0.3 0.015 250)",
    ring: "oklch(0.65 0.15 230)",
    sidebar: "oklch(0.18 0.015 250)",
    sidebarForeground: "oklch(0.85 0.015 250)",
    sidebarPrimary: "oklch(0.65 0.15 230)",
    sidebarPrimaryForeground: "oklch(0.16 0.015 250)",
    sidebarAccent: "oklch(0.25 0.015 250)",
    sidebarAccentForeground: "oklch(0.85 0.015 250)",
    sidebarBorder: "oklch(0.3 0.015 250)",
    sidebarRing: "oklch(0.65 0.15 230)",
    chart1: "oklch(0.65 0.2 15)",
    chart2: "oklch(0.7 0.12 55)",
    chart3: "oklch(0.7 0.12 140)",
    chart4: "oklch(0.65 0.15 230)",
    chart5: "oklch(0.65 0.12 290)",
  },
  previewColors: ["#0D1117", "#58A6FF", "#3FB950", "#F85149", "#A371F7", "#C9D1D9"],
}

// Dracula
export const dracula: ColorScheme = {
  id: "dracula",
  name: "Dracula",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.22 0.02 270)", // #282A36
    foreground: "oklch(0.92 0.01 270)", // #F8F8F2
    card: "oklch(0.22 0.02 270)",
    cardForeground: "oklch(0.92 0.01 270)",
    popover: "oklch(0.22 0.02 270)",
    popoverForeground: "oklch(0.92 0.01 270)",
    primary: "oklch(0.7 0.2 300)", // #BD93F9 purple
    primaryForeground: "oklch(0.22 0.02 270)",
    secondary: "oklch(0.32 0.02 270)", // #44475A
    secondaryForeground: "oklch(0.92 0.01 270)",
    muted: "oklch(0.38 0.02 270)",
    mutedForeground: "oklch(0.7 0.02 270)",
    accent: "oklch(0.75 0.15 180)", // #8BE9FD cyan
    accentForeground: "oklch(0.22 0.02 270)",
    destructive: "oklch(0.7 0.2 5)", // #FF5555 red
    destructiveForeground: "oklch(0.22 0.02 270)",
    border: "oklch(0.38 0.02 270)",
    input: "oklch(0.38 0.02 270)",
    ring: "oklch(0.7 0.2 300)",
    sidebar: "oklch(0.2 0.02 270)",
    sidebarForeground: "oklch(0.92 0.01 270)",
    sidebarPrimary: "oklch(0.7 0.2 300)",
    sidebarPrimaryForeground: "oklch(0.22 0.02 270)",
    sidebarAccent: "oklch(0.32 0.02 270)",
    sidebarAccentForeground: "oklch(0.92 0.01 270)",
    sidebarBorder: "oklch(0.38 0.02 270)",
    sidebarRing: "oklch(0.7 0.2 300)",
    chart1: "oklch(0.7 0.2 5)", // red
    chart2: "oklch(0.75 0.15 55)", // orange
    chart3: "oklch(0.8 0.15 130)", // green
    chart4: "oklch(0.75 0.15 180)", // cyan
    chart5: "oklch(0.7 0.2 300)", // purple
  },
  previewColors: ["#282A36", "#BD93F9", "#50FA7B", "#FF5555", "#FF79C6", "#F8F8F2"],
}

// Nord
export const nord: ColorScheme = {
  id: "nord",
  name: "Nord",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.28 0.02 230)", // #2E3440
    foreground: "oklch(0.9 0.01 230)", // #ECEFF4
    card: "oklch(0.28 0.02 230)",
    cardForeground: "oklch(0.9 0.01 230)",
    popover: "oklch(0.28 0.02 230)",
    popoverForeground: "oklch(0.9 0.01 230)",
    primary: "oklch(0.7 0.1 220)", // #88C0D0 frost
    primaryForeground: "oklch(0.28 0.02 230)",
    secondary: "oklch(0.35 0.02 230)", // #3B4252
    secondaryForeground: "oklch(0.9 0.01 230)",
    muted: "oklch(0.42 0.02 230)", // #434C5E
    mutedForeground: "oklch(0.7 0.02 230)",
    accent: "oklch(0.75 0.08 180)", // #8FBCBB teal
    accentForeground: "oklch(0.28 0.02 230)",
    destructive: "oklch(0.65 0.15 15)", // #BF616A red
    destructiveForeground: "oklch(0.28 0.02 230)",
    border: "oklch(0.42 0.02 230)",
    input: "oklch(0.42 0.02 230)",
    ring: "oklch(0.7 0.1 220)",
    sidebar: "oklch(0.25 0.02 230)",
    sidebarForeground: "oklch(0.9 0.01 230)",
    sidebarPrimary: "oklch(0.7 0.1 220)",
    sidebarPrimaryForeground: "oklch(0.28 0.02 230)",
    sidebarAccent: "oklch(0.35 0.02 230)",
    sidebarAccentForeground: "oklch(0.9 0.01 230)",
    sidebarBorder: "oklch(0.42 0.02 230)",
    sidebarRing: "oklch(0.7 0.1 220)",
    chart1: "oklch(0.65 0.15 15)", // red
    chart2: "oklch(0.75 0.12 55)", // orange
    chart3: "oklch(0.75 0.12 140)", // green
    chart4: "oklch(0.7 0.1 220)", // frost
    chart5: "oklch(0.65 0.12 280)", // purple
  },
  previewColors: ["#2E3440", "#88C0D0", "#A3BE8C", "#BF616A", "#B48EAD", "#ECEFF4"],
}

// Monokai Pro
export const monokaiPro: ColorScheme = {
  id: "monokai-pro",
  name: "Monokai Pro",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.23 0.015 60)", // #2D2A2E
    foreground: "oklch(0.92 0.01 90)", // #FCFCFA
    card: "oklch(0.23 0.015 60)",
    cardForeground: "oklch(0.92 0.01 90)",
    popover: "oklch(0.23 0.015 60)",
    popoverForeground: "oklch(0.92 0.01 90)",
    primary: "oklch(0.75 0.15 55)", // #FFD866 yellow
    primaryForeground: "oklch(0.23 0.015 60)",
    secondary: "oklch(0.3 0.015 60)", // #403E41
    secondaryForeground: "oklch(0.92 0.01 90)",
    muted: "oklch(0.38 0.015 60)",
    mutedForeground: "oklch(0.65 0.02 60)",
    accent: "oklch(0.75 0.12 175)", // #78DCE8 cyan
    accentForeground: "oklch(0.23 0.015 60)",
    destructive: "oklch(0.7 0.2 15)", // #FF6188 red
    destructiveForeground: "oklch(0.23 0.015 60)",
    border: "oklch(0.38 0.015 60)",
    input: "oklch(0.38 0.015 60)",
    ring: "oklch(0.75 0.15 55)",
    sidebar: "oklch(0.2 0.015 60)",
    sidebarForeground: "oklch(0.92 0.01 90)",
    sidebarPrimary: "oklch(0.75 0.15 55)",
    sidebarPrimaryForeground: "oklch(0.23 0.015 60)",
    sidebarAccent: "oklch(0.3 0.015 60)",
    sidebarAccentForeground: "oklch(0.92 0.01 90)",
    sidebarBorder: "oklch(0.38 0.015 60)",
    sidebarRing: "oklch(0.75 0.15 55)",
    chart1: "oklch(0.7 0.2 15)", // red
    chart2: "oklch(0.75 0.15 55)", // yellow
    chart3: "oklch(0.8 0.15 140)", // green
    chart4: "oklch(0.75 0.12 175)", // cyan
    chart5: "oklch(0.7 0.15 290)", // purple
  },
  previewColors: ["#2D2A2E", "#FFD866", "#A9DC76", "#FF6188", "#AB9DF2", "#FCFCFA"],
}

// Gruvbox Dark
export const gruvboxDark: ColorScheme = {
  id: "gruvbox-dark",
  name: "Gruvbox Dark",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.27 0.04 75)", // #282828
    foreground: "oklch(0.88 0.05 90)", // #EBDBB2
    card: "oklch(0.27 0.04 75)",
    cardForeground: "oklch(0.88 0.05 90)",
    popover: "oklch(0.27 0.04 75)",
    popoverForeground: "oklch(0.88 0.05 90)",
    primary: "oklch(0.75 0.15 55)", // #FABD2F yellow
    primaryForeground: "oklch(0.27 0.04 75)",
    secondary: "oklch(0.35 0.04 75)", // #3C3836
    secondaryForeground: "oklch(0.88 0.05 90)",
    muted: "oklch(0.42 0.04 75)",
    mutedForeground: "oklch(0.65 0.04 85)",
    accent: "oklch(0.7 0.12 175)", // #8EC07C aqua
    accentForeground: "oklch(0.27 0.04 75)",
    destructive: "oklch(0.6 0.2 25)", // #FB4934 red
    destructiveForeground: "oklch(0.27 0.04 75)",
    border: "oklch(0.42 0.04 75)",
    input: "oklch(0.42 0.04 75)",
    ring: "oklch(0.75 0.15 55)",
    sidebar: "oklch(0.24 0.04 75)",
    sidebarForeground: "oklch(0.88 0.05 90)",
    sidebarPrimary: "oklch(0.75 0.15 55)",
    sidebarPrimaryForeground: "oklch(0.27 0.04 75)",
    sidebarAccent: "oklch(0.35 0.04 75)",
    sidebarAccentForeground: "oklch(0.88 0.05 90)",
    sidebarBorder: "oklch(0.42 0.04 75)",
    sidebarRing: "oklch(0.75 0.15 55)",
    chart1: "oklch(0.6 0.2 25)", // red
    chart2: "oklch(0.7 0.15 55)", // orange
    chart3: "oklch(0.75 0.15 125)", // green
    chart4: "oklch(0.65 0.12 230)", // blue
    chart5: "oklch(0.65 0.15 300)", // purple
  },
  previewColors: ["#282828", "#FABD2F", "#B8BB26", "#FB4934", "#D3869B", "#EBDBB2"],
}

// Gruvbox Light
export const gruvboxLight: ColorScheme = {
  id: "gruvbox-light",
  name: "Gruvbox Light",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.95 0.03 90)", // #FBF1C7
    foreground: "oklch(0.35 0.05 75)", // #3C3836
    card: "oklch(0.95 0.03 90)",
    cardForeground: "oklch(0.35 0.05 75)",
    popover: "oklch(0.95 0.03 90)",
    popoverForeground: "oklch(0.35 0.05 75)",
    primary: "oklch(0.55 0.15 55)", // #D79921 yellow
    primaryForeground: "oklch(0.95 0.03 90)",
    secondary: "oklch(0.9 0.03 90)",
    secondaryForeground: "oklch(0.35 0.05 75)",
    muted: "oklch(0.85 0.03 90)",
    mutedForeground: "oklch(0.5 0.04 75)",
    accent: "oklch(0.55 0.12 175)", // #689D6A aqua
    accentForeground: "oklch(0.95 0.03 90)",
    destructive: "oklch(0.5 0.2 25)", // #CC241D red
    destructiveForeground: "oklch(0.95 0.03 90)",
    border: "oklch(0.82 0.03 90)",
    input: "oklch(0.82 0.03 90)",
    ring: "oklch(0.55 0.15 55)",
    sidebar: "oklch(0.92 0.03 90)",
    sidebarForeground: "oklch(0.35 0.05 75)",
    sidebarPrimary: "oklch(0.55 0.15 55)",
    sidebarPrimaryForeground: "oklch(0.95 0.03 90)",
    sidebarAccent: "oklch(0.9 0.03 90)",
    sidebarAccentForeground: "oklch(0.35 0.05 75)",
    sidebarBorder: "oklch(0.82 0.03 90)",
    sidebarRing: "oklch(0.55 0.15 55)",
    chart1: "oklch(0.5 0.2 25)", // red
    chart2: "oklch(0.6 0.15 55)", // orange
    chart3: "oklch(0.6 0.15 125)", // green
    chart4: "oklch(0.5 0.12 230)", // blue
    chart5: "oklch(0.5 0.15 300)", // purple
  },
  previewColors: ["#FBF1C7", "#D79921", "#98971A", "#CC241D", "#B16286", "#3C3836"],
}

// Rosé Pine
export const rosePine: ColorScheme = {
  id: "rose-pine",
  name: "Rosé Pine",
  mode: "dark",
  isBuiltin: true,
  palette: {
    background: "oklch(0.22 0.03 280)", // #191724
    foreground: "oklch(0.9 0.02 280)", // #E0DEF4
    card: "oklch(0.22 0.03 280)",
    cardForeground: "oklch(0.9 0.02 280)",
    popover: "oklch(0.22 0.03 280)",
    popoverForeground: "oklch(0.9 0.02 280)",
    primary: "oklch(0.75 0.12 340)", // #EBBCBA rose
    primaryForeground: "oklch(0.22 0.03 280)",
    secondary: "oklch(0.28 0.03 280)", // #1F1D2E
    secondaryForeground: "oklch(0.9 0.02 280)",
    muted: "oklch(0.35 0.03 280)",
    mutedForeground: "oklch(0.65 0.03 280)",
    accent: "oklch(0.7 0.15 35)", // #F6C177 gold
    accentForeground: "oklch(0.22 0.03 280)",
    destructive: "oklch(0.65 0.18 15)", // #EB6F92 love
    destructiveForeground: "oklch(0.22 0.03 280)",
    border: "oklch(0.35 0.03 280)",
    input: "oklch(0.35 0.03 280)",
    ring: "oklch(0.75 0.12 340)",
    sidebar: "oklch(0.2 0.03 280)",
    sidebarForeground: "oklch(0.9 0.02 280)",
    sidebarPrimary: "oklch(0.75 0.12 340)",
    sidebarPrimaryForeground: "oklch(0.22 0.03 280)",
    sidebarAccent: "oklch(0.28 0.03 280)",
    sidebarAccentForeground: "oklch(0.9 0.02 280)",
    sidebarBorder: "oklch(0.35 0.03 280)",
    sidebarRing: "oklch(0.75 0.12 340)",
    chart1: "oklch(0.65 0.18 15)", // love
    chart2: "oklch(0.7 0.15 35)", // gold
    chart3: "oklch(0.7 0.12 180)", // foam
    chart4: "oklch(0.7 0.12 280)", // iris
    chart5: "oklch(0.75 0.12 340)", // rose
  },
  previewColors: ["#191724", "#EBBCBA", "#9CCFD8", "#EB6F92", "#C4A7E7", "#E0DEF4"],
}

// Rosé Pine Dawn (Light)
export const rosePineDawn: ColorScheme = {
  id: "rose-pine-dawn",
  name: "Rosé Pine Dawn",
  mode: "light",
  isBuiltin: true,
  palette: {
    background: "oklch(0.97 0.01 50)", // #FAF4ED
    foreground: "oklch(0.4 0.04 280)", // #575279
    card: "oklch(0.97 0.01 50)",
    cardForeground: "oklch(0.4 0.04 280)",
    popover: "oklch(0.97 0.01 50)",
    popoverForeground: "oklch(0.4 0.04 280)",
    primary: "oklch(0.6 0.12 340)", // #D7827E rose
    primaryForeground: "oklch(0.97 0.01 50)",
    secondary: "oklch(0.94 0.01 50)",
    secondaryForeground: "oklch(0.4 0.04 280)",
    muted: "oklch(0.9 0.01 50)",
    mutedForeground: "oklch(0.55 0.03 280)",
    accent: "oklch(0.6 0.12 35)", // #EA9D34 gold
    accentForeground: "oklch(0.97 0.01 50)",
    destructive: "oklch(0.55 0.18 15)", // #B4637A love
    destructiveForeground: "oklch(0.97 0.01 50)",
    border: "oklch(0.88 0.01 50)",
    input: "oklch(0.88 0.01 50)",
    ring: "oklch(0.6 0.12 340)",
    sidebar: "oklch(0.94 0.01 50)",
    sidebarForeground: "oklch(0.4 0.04 280)",
    sidebarPrimary: "oklch(0.6 0.12 340)",
    sidebarPrimaryForeground: "oklch(0.97 0.01 50)",
    sidebarAccent: "oklch(0.92 0.01 50)",
    sidebarAccentForeground: "oklch(0.4 0.04 280)",
    sidebarBorder: "oklch(0.88 0.01 50)",
    sidebarRing: "oklch(0.6 0.12 340)",
    chart1: "oklch(0.55 0.18 15)", // love
    chart2: "oklch(0.6 0.12 35)", // gold
    chart3: "oklch(0.55 0.1 180)", // foam
    chart4: "oklch(0.5 0.1 280)", // iris
    chart5: "oklch(0.6 0.12 340)", // rose
  },
  previewColors: ["#FAF4ED", "#D7827E", "#56949F", "#B4637A", "#907AA9", "#575279"],
}

// Export all builtin schemes
export const builtinSchemes: ColorScheme[] = [
  defaultLight,
  defaultDark,
  dracula,
  nord,
  monokaiPro,
  gruvboxDark,
  gruvboxLight,
  rosePine,
  rosePineDawn,
  flexokiLight,
  flexokiDark,
  catppuccinLatte,
  catppuccinMocha,
  solarizedLight,
  solarizedDark,
  tokyoNightLight,
  tokyoNightDark,
  oneDarkPro,
  githubLight,
  githubDark,
]

export const lightSchemes = builtinSchemes.filter((s) => s.mode === "light")
export const darkSchemes = builtinSchemes.filter((s) => s.mode === "dark")

export function getSchemeById(id: string, mode: "light" | "dark"): ColorScheme | undefined {
  return builtinSchemes.find((s) => s.id === id && s.mode === mode)
}

export function getSchemeOrDefault(id: string, mode: "light" | "dark"): ColorScheme {
  const scheme = getSchemeById(id, mode)
  if (scheme) return scheme
  return mode === "light" ? defaultLight : defaultDark
}
