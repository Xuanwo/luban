import { LubanIDE } from "@/components/luban-ide"
import { AppearanceSync } from "@/components/appearance-sync"
import { GlobalZoomShortcuts } from "@/components/global-zoom-shortcuts"
import { LubanProvider } from "@/lib/luban-context"

export default function Home() {
  return (
    <LubanProvider>
      <AppearanceSync />
      <GlobalZoomShortcuts />
      <LubanIDE />
    </LubanProvider>
  )
}
