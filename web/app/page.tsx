import { AgentIDE } from "@/components/agent-ide"
import { LubanProvider } from "@/lib/luban-context"

export default function Home() {
  return (
    <LubanProvider>
      <AgentIDE />
    </LubanProvider>
  )
}
