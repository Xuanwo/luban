export type ColorSchemeMode = "light" | "dark"

export type ColorSchemeId = string

export interface ColorPalette {
  background: string
  foreground: string
  card: string
  cardForeground: string
  popover: string
  popoverForeground: string
  primary: string
  primaryForeground: string
  secondary: string
  secondaryForeground: string
  muted: string
  mutedForeground: string
  accent: string
  accentForeground: string
  destructive: string
  destructiveForeground: string
  border: string
  input: string
  ring: string
  sidebar: string
  sidebarForeground: string
  sidebarPrimary: string
  sidebarPrimaryForeground: string
  sidebarAccent: string
  sidebarAccentForeground: string
  sidebarBorder: string
  sidebarRing: string
  chart1: string
  chart2: string
  chart3: string
  chart4: string
  chart5: string
}

export interface ColorScheme {
  id: ColorSchemeId
  name: string
  mode: ColorSchemeMode
  isBuiltin: boolean
  palette: ColorPalette
  previewColors: string[]
}

export interface ColorSchemeSettings {
  lightSchemeId: ColorSchemeId
  darkSchemeId: ColorSchemeId
  customSchemes: ColorScheme[]
}

export const defaultColorSchemeSettings: ColorSchemeSettings = {
  lightSchemeId: "default",
  darkSchemeId: "default",
  customSchemes: [],
}
