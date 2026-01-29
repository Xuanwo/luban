"use client"

import type React from "react"

import { useState, useRef, useEffect, useCallback } from "react"
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Clock,
  Wrench,
  Brain,
  FileCode,
  ArrowDown,
  MessageSquare,
  Plus,
  X,
  ExternalLink,
  GitBranch,
  RotateCcw,
  Paperclip,
  FileDiffIcon,
  Columns2,
  AlignJustify,
  Pencil,
  Sparkles,
  Check,
  Loader2,
  Keyboard,
} from "lucide-react"
import { cn } from "@/lib/utils"
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core"
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable"
import { CSS } from "@dnd-kit/utilities"
import { MultiFileDiff, type FileContents } from "@pierre/diffs/react"
import { type ChangedFile, mockChangedFiles } from "./right-sidebar"
import { AgentIcon } from "./shared/agent-selector"
import { ActivityStream, AgentRunningCard, type AgentRunningStatus } from "./shared/activity-item"
import type { ChatMessage as Message, MessageAttachment } from "./shared/chat-message"
import { MessageEditor } from "./shared/message-editor"
import { OpenButton } from "./shared/open-button"

interface DiffFileData {
  file: ChangedFile
  oldFile: FileContents
  newFile: FileContents
}

interface ChatTab {
  id: string
  title: string
  isActive: boolean
  type: "conversation" | "diff"
  // For diff tabs - now stores all files
  diffData?: {
    files: DiffFileData[]
    activeFileId?: string
  }
}

interface ClosedTab {
  id: string
  title: string
  closedAt: Date
  type: "conversation" | "diff"
}





// Queued messages for tab 1 (with queue)
const queuedMessagesWithQueue: Message[] = [
  {
    id: "q1",
    type: "user",
    content: "请帮我添加一个暗色主题的切换按钮",
    isQueued: true,
    queuePosition: 1,
  },
  {
    id: "q2",
    type: "user",
    content: "优化一下移动端的响应式布局",
    isQueued: true,
    queuePosition: 2,
  },
]

// No queued messages for tab 2
const queuedMessagesEmpty: Message[] = []

const sampleMessages: Message[] = [
  {
    id: "0",
    type: "user",
    content: `请帮我实现一个新的滚动行为：当用户切换聊天时，自动滚动到底部`,
  },
  {
    id: "1",
    type: "assistant",
    content: `**实现要点**

• 在 chat_target_changed 时强制 follow=true + 安排一次 "scroll to bottom"
• 移除切换时保存/恢复滚动位置的逻辑（WorkspaceChatScrollSaved/AnchorSaved 不再由 UI 触发）
• flush_pending_chat_scroll_to_bottom 增加 "等待布局稳定" 逻辑，避免短历史时滚进底部 padding

**改动位置**`,
    timestamp: "56m28s",
    activities: [
      { id: "a1", type: "thinking", title: "Analyzing scroll behavior requirements", status: "done", duration: "12s" },
      { id: "a2", type: "search", title: "Searching for chat_target_changed usages", status: "done", duration: "3s" },
      {
        id: "a3",
        type: "file_edit",
        title: "crates/luban_ui/src/root.rs",
        detail: "Modified scroll handling logic at line 377",
        status: "done",
        duration: "8s",
      },
      {
        id: "a4",
        type: "file_edit",
        title: "crates/luban_ui/src/root/dashboard.rs",
        detail: "Updated follow state management",
        status: "done",
        duration: "5s",
      },
      {
        id: "a5",
        type: "shell",
        shellType: "zsh",
        title: "just fmt && just lint && just test",
        detail: "All checks passed",
        status: "done",
        duration: "45s",
      },
    ],
    metadata: {
      toolCalls: 18,
      thinkingSteps: 32,
      duration: "7m36s",
    },
    codeReferences: [
      { file: "crates/luban_ui/src/root.rs", line: 377 },
      { file: "crates/luban_ui/src/root/dashboard.rs", line: 417 },
      { file: "crates/luban_ui/src/root/tests.rs", line: 3276 },
    ],
  },
  {
    id: "2",
    type: "user",
    content: `现在我们的图片渲染非常奇怪，有以下几个问题
• 往上翻滚的时候进度条会跳来跳去
• 图片被意外的裁切了，我觉得我们可以替换成缩略图而不是原始的图片，原始的图片在用户点击的时候再加载`,
  },
  {
    id: "3",
    type: "assistant",
    content: `我来分析一下图片渲染的问题。

**问题根因**

1. **滚动跳动**: 图片加载完成后高度变化导致 layout shift
2. **裁切问题**: 当前使用 \`object-fit: cover\` 会裁切超出容器的部分

**解决方案**

\`\`\`rust
// 使用 aspect-ratio 预留空间
fn image_placeholder(width: u32, height: u32) -> impl Element {
    div()
        .style(|s| s
            .aspect_ratio(width as f32 / height as f32)
            .bg(theme::colors::surface_muted)
        )
}
\`\`\`

**缩略图策略**

我建议实现一个两阶段加载：
1. 先加载 blur hash 或低分辨率缩略图
2. 用户点击时加载原图，支持缩放和平移`,
    timestamp: "42m15s",
    activities: [
      { id: "b1", type: "thinking", title: "Analyzing image rendering pipeline", status: "done", duration: "15s" },
      { id: "b2", type: "search", title: "Finding image loading patterns", status: "done", duration: "4s" },
      { id: "b3", type: "file_edit", title: "crates/luban_ui/src/image.rs", status: "done", duration: "12s" },
      {
        id: "b4",
        type: "file_edit",
        title: "crates/luban_ui/src/components/thumbnail.rs",
        status: "done",
        duration: "8s",
      },
    ],
    metadata: {
      toolCalls: 12,
      thinkingSteps: 24,
      duration: "5m22s",
    },
  },
  {
    id: "4",
    type: "user",
    content: `很好！请继续实现 blur hash 的支持`,
  },
  {
    id: "5",
    type: "assistant",
    content: `已添加 blurhash 支持。

**新增依赖**

\`\`\`toml
[dependencies]
blurhash = "0.2"
\`\`\`

**实现细节**

- 上传时计算 blurhash 并存储在元数据中
- 渲染时先显示 blurhash 解码的模糊图
- 原图加载完成后平滑过渡

**测试结果**

✓ 滚动不再跳动
✓ 图片显示完整
✓ 加载体验流畅`,
    timestamp: "38m42s",
    activities: [
      { id: "c1", type: "thinking", title: "Planning blurhash integration", status: "done", duration: "8s" },
      { id: "c2", type: "shell", shellType: "bash", title: "cargo add blurhash", status: "done", duration: "3s" },
      {
        id: "c3",
        type: "file_edit",
        title: "crates/luban_core/src/image/blurhash.rs",
        status: "done",
        duration: "18s",
      },
      { id: "c4", type: "file_edit", title: "crates/luban_ui/src/image.rs", status: "done", duration: "10s" },
      { id: "c5", type: "shell", shellType: "zsh", title: "cargo test image::", status: "done", duration: "25s" },
    ],
    metadata: {
      toolCalls: 15,
      thinkingSteps: 28,
      duration: "6m15s",
    },
  },
  {
    id: "6",
    type: "user",
    content: `现在请帮我优化一下键盘快捷键的处理，目前按键响应有延迟`,
  },
  {
    id: "7",
    type: "assistant",
    content: `我来分析键盘事件的处理流程。

**性能分析**

当前问题：
- 键盘事件在主线程处理，被渲染阻塞
- 每次按键都触发完整的 action dispatch 流程
- 快捷键匹配使用线性查找

**优化方案**

1. **事件预处理**: 在事件循环早期拦截快捷键
2. **快捷键索引**: 使用 HashMap 替代 Vec 遍历
3. **防抖处理**: 对重复按键进行合并

\`\`\`rust
struct KeyboardHandler {
    shortcuts: HashMap<KeyBinding, Action>,
    pending_keys: VecDeque<KeyEvent>,
    debounce_timer: Option<Timer>,
}
\`\`\``,
    timestamp: "25m18s",
    activities: [
      { id: "d1", type: "thinking", title: "Profiling keyboard event handling", status: "done", duration: "20s" },
      { id: "d2", type: "search", title: "Finding keyboard handler code", status: "done", duration: "2s" },
      { id: "d3", type: "file_edit", title: "crates/luban_input/src/keyboard.rs", status: "done", duration: "22s" },
      { id: "d4", type: "shell", shellType: "fish", title: "cargo bench keyboard", status: "done", duration: "15s" },
    ],
    metadata: {
      toolCalls: 10,
      thinkingSteps: 18,
      duration: "4m45s",
    },
  },
  {
    id: "8",
    type: "user",
    content: `帮我实现一个 vim 模式的文本编辑器`,
  },
  {
    id: "9",
    type: "assistant",
    content: "",
    isStreaming: true,
    activities: [
      { 
        id: "s1", 
        type: "thinking", 
        title: "Analyzing vim mode requirements", 
        status: "done", 
        duration: "8s",
        detail: "User wants to implement vim mode for the text editor. This requires:\n- Normal/Insert/Visual mode handling\n- Key binding system\n- Cursor movement commands\n- Text manipulation operations"
      },
      { 
        id: "s2", 
        type: "search", 
        title: "Finding editor component implementations", 
        status: "done", 
        duration: "2s",
        detail: "Found 3 files:\n- crates/luban_editor/src/lib.rs\n- crates/luban_editor/src/buffer.rs\n- crates/luban_editor/src/cursor.rs"
      },
      { 
        id: "s3", 
        type: "shell", 
        shellType: "zsh",
        title: "cargo check --package luban_editor", 
        status: "done", 
        duration: "3s",
	        detail: "$ cargo check --package luban_editor\n    Checking luban_editor v0.1.6\n    Finished dev [unoptimized + debuginfo] target(s) in 2.84s"
      },
      { 
        id: "s4", 
        type: "file_edit", 
        title: "crates/luban_editor/src/vim/mod.rs", 
        status: "running",
        detail: "Creating vim mode module with:\n- VimMode enum (Normal, Insert, Visual)\n- VimState struct\n- Key handler implementation"
      },
    ],
  },
]

interface ChatPanelProps {
  // Callback when a diff tab should be opened from external sources (e.g., Changes panel)
  pendingDiffFile?: ChangedFile | null
  onDiffFileOpened?: () => void
}

export function ChatPanel({ pendingDiffFile, onDiffFileOpened }: ChatPanelProps) {
  const [input, setInput] = useState("")
  const [showScrollButton, setShowScrollButton] = useState(true)
  const scrollContainerRef = useRef<HTMLDivElement>(null)
  
  const [showTabDropdown, setShowTabDropdown] = useState(false)

  const [attachments, setAttachments] = useState<MessageAttachment[]>([])
  
  // Message history for ↑↓ navigation
  const [messageHistory, setMessageHistory] = useState<string[]>([
    "请帮我实现一个新的滚动行为：当用户切换聊天时，自动滚动到底部",
    "现在我们的图片渲染非常奇怪，有以下几个问题",
    "很好！请继续实现 blur hash 的支持",
    "现在请帮我优化一下键盘快捷键的处理，目前按键响应有延迟",
    "帮我实现一个 vim 模式的文本编辑器",
  ])

  const [diffStyle, setDiffStyle] = useState<"split" | "unified">("split")

  useEffect(() => {
    const container = scrollContainerRef.current
    if (!container) return

    const handleScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = container
      const isAtBottom = scrollHeight - scrollTop - clientHeight < 50
      setShowScrollButton(!isAtBottom)
    }

    container.addEventListener("scroll", handleScroll)
    handleScroll()
    return () => container.removeEventListener("scroll", handleScroll)
  }, [])

  const scrollToBottom = () => {
    scrollContainerRef.current?.scrollTo({
      top: scrollContainerRef.current.scrollHeight,
      behavior: "smooth",
    })
  }

  const [projectInfo, setProjectInfo] = useState({
    name: "luban",
    branch: "feat/add-branch-rename",
  })

  const [isRenamingBranch, setIsRenamingBranch] = useState(false)
  const [branchRenameValue, setBranchRenameValue] = useState("")
  const [isAiRenaming, setIsAiRenaming] = useState(false)
  const [copySuccess, setCopySuccess] = useState(false)
  const branchInputRef = useRef<HTMLInputElement>(null)

  const isMainBranch = projectInfo.branch === "main"

  const handleStartRename = () => {
    setBranchRenameValue(projectInfo.branch)
    setIsRenamingBranch(true)
  }

  const handleConfirmRename = () => {
    if (branchRenameValue.trim() && branchRenameValue !== projectInfo.branch) {
      setProjectInfo((prev) => ({ ...prev, branch: branchRenameValue.trim() }))
    }
    setIsRenamingBranch(false)
  }

  const handleCancelRename = () => {
    setIsRenamingBranch(false)
    setBranchRenameValue("")
  }

  const handleAiRename = () => {
    setIsAiRenaming(true)
    setTimeout(() => {
      const suggestions = ["feat/add-auth-middleware", "fix/resolve-scroll-issue", "refactor/cleanup-sidebar"]
      const newName = suggestions[Math.floor(Math.random() * suggestions.length)]
      setProjectInfo((prev) => ({ ...prev, branch: newName }))
      setIsAiRenaming(false)
    }, 1200)
  }

  const handleCopyBranch = async () => {
    await navigator.clipboard.writeText(projectInfo.branch)
    setCopySuccess(true)
    setTimeout(() => setCopySuccess(false), 1500)
  }

  useEffect(() => {
    if (isRenamingBranch && branchInputRef.current) {
      branchInputRef.current.focus()
      branchInputRef.current.select()
    }
  }, [isRenamingBranch])

  const [tabs, setTabs] = useState<ChatTab[]>([
    { id: "1", title: "有排队消息 (ESC→暂停)", isActive: true, type: "conversation" },
    { id: "2", title: "无排队消息 (ESC→取消)", isActive: false, type: "conversation" },
  ])
  const [activeTabId, setActiveTabId] = useState("1")

  const [closedTabs, setClosedTabs] = useState<ClosedTab[]>([
    { id: "old-1", title: "滚动位置修复", closedAt: new Date("2026-01-14T00:00:00.000Z"), type: "conversation" },
    { id: "old-2", title: "UI 布局讨论", closedAt: new Date("2026-01-13T22:00:00.000Z"), type: "conversation" },
  ])

  // Different queue data per tab
  const getInitialQueue = (tabId: string) => tabId === "1" ? queuedMessagesWithQueue : queuedMessagesEmpty
  const [messageQueue, setMessageQueue] = useState<Message[]>(getInitialQueue(activeTabId))
  
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null)
  const [editingContent, setEditingContent] = useState("")
  const [editingAttachments, setEditingAttachments] = useState<MessageAttachment[]>([])
  
  // Agent running state
  const [agentRunningStatus, setAgentRunningStatus] = useState<AgentRunningStatus>("running")
  const [agentEditorValue, setAgentEditorValue] = useState("")
  const [agentEditorAttachments, setAgentEditorAttachments] = useState<MessageAttachment[]>([])
  const hasQueuedMessages = messageQueue.length > 0

  // Esc key double-press state
  const [escHintVisible, setEscHintVisible] = useState(false)
  const escTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const ESC_TIMEOUT_MS = 2000

  const handleCancelMessage = (messageId: string) => {
    setMessageQueue((prev) => {
      const filtered = prev.filter((msg) => msg.id !== messageId)
      return filtered.map((msg, idx) => ({
        ...msg,
        queuePosition: idx + 1,
      }))
    })
  }

  const handleStartEdit = (message: Message) => {
    setEditingMessageId(message.id)
    setEditingContent(message.content)
    setEditingAttachments(message.attachments || [])
  }

  const handleSaveEdit = () => {
    if (editingMessageId && (editingContent.trim() || editingAttachments.length > 0)) {
      setMessageQueue((prev) =>
        prev.map((msg) =>
          msg.id === editingMessageId
            ? { ...msg, content: editingContent.trim(), attachments: editingAttachments }
            : msg,
        ),
      )
    }
    setEditingMessageId(null)
    setEditingContent("")
    setEditingAttachments([])
  }

  const handleCancelEdit = () => {
    setEditingMessageId(null)
    setEditingContent("")
    setEditingAttachments([])
  }

  // Agent running cancel/resume handlers
  const handleAgentCancel = () => {
    setAgentRunningStatus("cancelling")
  }

  const handleAgentResume = () => {
    setAgentRunningStatus("resuming")
  }

  const handleAgentSubmit = () => {
    if (!agentEditorValue.trim() && agentEditorAttachments.length === 0) return
    
    const isResuming = agentRunningStatus === "resuming"
    console.log(isResuming ? "Resume with message:" : "Cancel with new message:", agentEditorValue, agentEditorAttachments)
    
    // Mark current turn as cancelled and start new agent running
    setAgentRunningStatus("cancelled")
    setAgentEditorValue("")
    setAgentEditorAttachments([])
    
    // In a real implementation, this would submit the new message and start a new turn
    setTimeout(() => {
      setAgentRunningStatus("running")
    }, 500)
  }

  const handleAgentDismiss = () => {
    if (hasQueuedMessages) {
      // Has queued messages: always transition to paused state (user must handle queue)
      setAgentRunningStatus("paused")
    } else {
      // No queued messages: directly cancel
      setAgentRunningStatus("cancelled")
    }
    setAgentEditorValue("")
    setAgentEditorAttachments([])
  }

  // Clear Esc hint and timeout
  const clearEscHint = useCallback(() => {
    setEscHintVisible(false)
    if (escTimeoutRef.current) {
      clearTimeout(escTimeoutRef.current)
      escTimeoutRef.current = null
    }
  }, [])

  // Handle Esc key press for cancelling agent
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return
      
      // Only handle when agent is actively running
      if (agentRunningStatus !== "running") return
      
      // Don't trigger if in an input or editing state
      const target = e.target as HTMLElement
      const isInInput = target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable
      
      // Allow Esc in our own message editor when it's already focused and cancelling
      if (isInInput && agentRunningStatus === "running") {
        // First Esc from any input - show hint, don't prevent default
        // This lets the input handle its own Esc behavior first
      }
      
      if (escHintVisible) {
        // Second press - cancel the agent
        e.preventDefault()
        clearEscHint()
        handleAgentCancel()
      } else {
        // First press - show hint
        e.preventDefault()
        setEscHintVisible(true)
        escTimeoutRef.current = setTimeout(() => {
          setEscHintVisible(false)
        }, ESC_TIMEOUT_MS)
      }
    }
    
    window.addEventListener("keydown", handleKeyDown)
    return () => {
      window.removeEventListener("keydown", handleKeyDown)
      if (escTimeoutRef.current) {
        clearTimeout(escTimeoutRef.current)
      }
    }
  }, [agentRunningStatus, escHintVisible, clearEscHint])

  // Clear hint when agent status changes
  useEffect(() => {
    if (agentRunningStatus !== "running") {
      clearEscHint()
    }
  }, [agentRunningStatus, clearEscHint])

  const sensors = useSensors(
    useSensor(PointerSensor, {
      activationConstraint: {
        distance: 8,
      },
    }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  )

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event
    if (over && active.id !== over.id) {
      setMessageQueue((items) => {
        const oldIndex = items.findIndex((item) => item.id === active.id)
        const newIndex = items.findIndex((item) => item.id === over.id)
        return arrayMove(items, oldIndex, newIndex)
      })
    }
  }

  const generateMockDiffData = (file: ChangedFile): DiffFileData => {
    const oldFile: FileContents = {
      name: file.name,
      contents:
        file.status === "added"
          ? ""
          : `// Old content of ${file.name}
export function example() {
  console.log("Hello");
  return 42;
}

export const config = {
  debug: true,
  version: "1.0.0"
};`,
    }

    const newFile: FileContents = {
      name: file.name,
      contents:
        file.status === "deleted"
          ? ""
          : `// New content of ${file.name}
export function example() {
  console.log("Hello, World!");
  return 100;
}

export const config = {
  debug: false,
  version: "1.1.0",
  newOption: true
};`,
    }

    return { file, oldFile, newFile }
  }

  useEffect(() => {
    if (!pendingDiffFile) return

    const targetFile = pendingDiffFile
    const existingDiffTab = tabs.find((t) => t.type === "diff")

    if (existingDiffTab) {
      setTabs(
        tabs.map((t) => ({
          ...t,
          isActive: t.id === existingDiffTab.id,
          diffData: t.id === existingDiffTab.id ? { ...t.diffData!, activeFileId: targetFile.id } : t.diffData,
        })),
      )
      setActiveTabId(existingDiffTab.id)
      onDiffFileOpened?.()
      return
    }

    const allFilesData: DiffFileData[] = mockChangedFiles.map(generateMockDiffData)
    const newTab: ChatTab = {
      id: `diff-all-${Date.now()}`,
      title: "Changes",
      isActive: true,
      type: "diff",
      diffData: {
        files: allFilesData,
        activeFileId: targetFile.id,
      },
    }

    setTabs([...tabs.map((t) => ({ ...t, isActive: false })), newTab])
    setActiveTabId(newTab.id)
    onDiffFileOpened?.()
  }, [pendingDiffFile, onDiffFileOpened, tabs])

  const handleTabClick = (tabId: string) => {
    setActiveTabId(tabId)
    setTabs(tabs.map((t) => ({ ...t, isActive: t.id === tabId })))
    // Reset agent state and load queue for the new tab
    setMessageQueue(getInitialQueue(tabId))
    setAgentRunningStatus("running")
    setAgentEditorValue("")
    setAgentEditorAttachments([])
  }

  const handleCloseTab = (tabId: string, e: React.MouseEvent) => {
    e.stopPropagation()
    if (tabs.length <= 1) return

    const closingTab = tabs.find((t) => t.id === tabId)
    if (closingTab) {
      setClosedTabs([
        { id: closingTab.id, title: closingTab.title, closedAt: new Date(), type: closingTab.type },
        ...closedTabs,
      ])
    }

    const newTabs = tabs.filter((t) => t.id !== tabId)
    if (activeTabId === tabId) {
      setActiveTabId(newTabs[0].id)
      newTabs[0].isActive = true
    }
    setTabs(newTabs)
  }

  const handleAddTab = () => {
    const newTab: ChatTab = {
      id: Date.now().toString(),
      title: "New Chat",
      isActive: true,
      type: "conversation",
    }
    setTabs([...tabs.map((t) => ({ ...t, isActive: false })), newTab])
    setActiveTabId(newTab.id)
  }

  const handleRestoreTab = (closedTab: ClosedTab) => {
    const restoredTab: ChatTab = {
      id: closedTab.id,
      title: closedTab.title,
      isActive: true,
      type: closedTab.type,
    }
    setTabs([...tabs.map((t) => ({ ...t, isActive: false })), restoredTab])
    setActiveTabId(restoredTab.id)
    setClosedTabs(closedTabs.filter((t) => t.id !== closedTab.id))
    setShowTabDropdown(false)
  }

  const activeTab = tabs.find((t) => t.id === activeTabId)
  const isDiffTab = activeTab?.type === "diff"

  return (
    <div className="flex-1 flex flex-col min-w-0 bg-background">
      <div className="flex items-center h-11 border-b border-border bg-card px-4">
        <div className="flex items-center gap-2 min-w-0 flex-1">
          <span className="text-sm font-medium text-foreground truncate">{projectInfo.name}</span>
          <div className="group/branch relative flex items-center gap-1 text-muted-foreground">
            <GitBranch className="w-3.5 h-3.5" />
            {isRenamingBranch ? (
              <div className="flex items-center gap-1">
                <input
                  ref={branchInputRef}
                  type="text"
                  value={branchRenameValue}
                  onChange={(e) => setBranchRenameValue(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleConfirmRename()
                    if (e.key === "Escape") handleCancelRename()
                  }}
                  onBlur={handleConfirmRename}
                  className="text-xs bg-muted border border-border rounded px-1.5 py-0.5 w-40 focus:outline-none focus:ring-1 focus:ring-primary"
                />
                <button
                  onMouseDown={(e) => e.preventDefault()}
                  onClick={handleConfirmRename}
                  className="p-0.5 text-muted-foreground hover:text-primary transition-colors"
                  title="Confirm"
                >
                  <Check className="w-3 h-3" />
                </button>
              </div>
            ) : (
              <>
                <span className="text-xs">{projectInfo.branch}</span>
                {isAiRenaming ? (
                  <Loader2 className="w-3 h-3 animate-spin text-primary ml-1" />
                ) : (
                  <div className="absolute right-0 top-1/2 -translate-y-1/2 z-10 flex items-center gap-0.5 opacity-0 group-hover/branch:opacity-100 transition-opacity bg-card px-0.5">
                    {!isMainBranch && (
                      <>
                        <button
                          onClick={handleStartRename}
                          className="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                          title="Rename branch"
                        >
                          <Pencil className="w-3 h-3" />
                        </button>
                        <button
                          onClick={handleAiRename}
                          className="p-0.5 text-muted-foreground hover:text-primary transition-colors"
                          title="AI rename"
                        >
                          <Sparkles className="w-3 h-3" />
                        </button>
                      </>
                    )}
                    <button
                      onClick={handleCopyBranch}
                      className="p-0.5 text-muted-foreground hover:text-foreground transition-colors"
                      title={copySuccess ? "Copied!" : "Copy branch name"}
                    >
                      {copySuccess ? (
                        <Check className="w-3 h-3 text-status-success" />
                      ) : (
                        <Copy className="w-3 h-3" />
                      )}
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
          <OpenButton />
        </div>
      </div>

      <div className="flex items-center h-11 border-b border-border bg-muted/30 px-1">
        <div className="flex-1 flex items-center gap-1 min-w-0 overflow-x-auto scrollbar-none py-1">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              onClick={() => handleTabClick(tab.id)}
              className={cn(
                "group relative flex items-center gap-2 h-8 px-3 cursor-pointer transition-all duration-200 min-w-0 max-w-[180px] rounded-lg",
                tab.id === activeTabId
                  ? "text-foreground bg-glass-surface shadow-[var(--shadow-glass)]"
                  : "text-muted-foreground hover:text-foreground hover:bg-glass-surface-muted/50",
              )}
            >
              {tab.type === "diff" ? (
                <FileDiffIcon className="w-3.5 h-3.5 flex-shrink-0 text-status-warning" />
              ) : (
                <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
              )}
              <span className="text-xs truncate flex-1">{tab.title}</span>
              {tabs.length > 1 && (
                <button
                  onClick={(e) => handleCloseTab(tab.id, e)}
                  className="p-0.5 opacity-0 group-hover:opacity-100 hover:bg-muted rounded transition-all"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          ))}
          <button
            onClick={handleAddTab}
            className="flex items-center justify-center w-8 h-10 text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors flex-shrink-0"
            title="New tab"
          >
            <Plus className="w-4 h-4" />
          </button>
        </div>

        <div className="flex items-center px-1">
          <div className="relative">
            <button
              onClick={() => setShowTabDropdown(!showTabDropdown)}
              className={cn(
                "flex items-center justify-center w-8 h-8 text-muted-foreground hover:text-foreground hover:bg-muted rounded transition-colors",
                showTabDropdown && "bg-muted text-foreground",
              )}
              title="All tabs"
            >
              <ChevronDown className="w-4 h-4" />
            </button>

            {showTabDropdown && (
              <>
                <div className="fixed inset-0 z-40" onClick={() => setShowTabDropdown(false)} />
                <div className="absolute right-0 top-full mt-1 w-64 bg-card border border-border rounded-lg shadow-xl z-50 overflow-hidden">
                  <div className="p-2 border-b border-border">
                    <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                      Open Tabs
                    </span>
                  </div>
                  <div className="max-h-40 overflow-y-auto">
                    {tabs.map((tab) => (
                      <button
                        key={tab.id}
                        onClick={() => {
                          handleTabClick(tab.id)
                          setShowTabDropdown(false)
                        }}
                        className={cn(
                          "w-full flex items-center gap-2 px-3 py-2 text-left text-xs hover:bg-muted transition-colors",
                          tab.id === activeTabId && "bg-primary/10 text-primary",
                        )}
                      >
                        {tab.type === "diff" ? (
                          <FileDiffIcon className="w-3.5 h-3.5 flex-shrink-0 text-status-warning" />
                        ) : (
                          <MessageSquare className="w-3.5 h-3.5 flex-shrink-0" />
                        )}
                        <span className="truncate">{tab.title}</span>
                      </button>
                    ))}
                  </div>

                  {closedTabs.length > 0 && (
                    <>
                      <div className="p-2 border-t border-border">
                        <span className="text-[10px] uppercase tracking-wider text-muted-foreground font-medium px-2">
                          Recently Closed
                        </span>
                      </div>
                      <div className="max-h-32 overflow-y-auto">
                        {closedTabs.map((tab) => (
                          <button
                            key={tab.id}
                            onClick={() => handleRestoreTab(tab)}
                            className="w-full flex items-center gap-2 px-3 py-2 text-left text-xs text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
                          >
                            <RotateCcw className="w-3.5 h-3.5 flex-shrink-0" />
                            <span className="truncate flex-1">{tab.title}</span>
                          </button>
                        ))}
                      </div>
                    </>
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      </div>

      {isDiffTab && activeTab?.diffData ? (
        <AllFilesDiffViewer
          files={activeTab.diffData.files}
          activeFileId={activeTab.diffData.activeFileId}
          diffStyle={diffStyle}
          onStyleChange={setDiffStyle}
        />
      ) : (
        <>
          <div ref={scrollContainerRef} className="flex-1 overflow-y-auto relative">
            <div className="max-w-3xl mx-auto py-4 px-4 pb-20 space-y-4">
              {sampleMessages.map((message) => (
                <div key={message.id} className="group">
                  {message.type === "assistant" ? (
                    <div className="space-y-1">
                      {message.activities && (
                        message.isStreaming && agentRunningStatus !== "cancelled" ? (
                          <AgentRunningCard
                            activities={message.activities}
                            elapsedTime="00:32"
                            status={agentRunningStatus}
                            hasQueuedMessages={hasQueuedMessages}
                            editorValue={agentEditorValue}
                            editorAttachments={agentEditorAttachments}
                            onEditorChange={setAgentEditorValue}
                            onEditorAttachmentsChange={setAgentEditorAttachments}
                            onCancel={handleAgentCancel}
                            onResume={handleAgentResume}
                            onSubmit={handleAgentSubmit}
                            onDismiss={handleAgentDismiss}
                          />
                        ) : (
                          <ActivityStream 
                            activities={message.activities} 
                            isStreaming={false} 
                            isCancelled={message.isStreaming && agentRunningStatus === "cancelled"}
                            variant="chat" 
                          />
                        )
                      )}

                      {message.content && (
                        <div className="text-[13px] leading-relaxed text-foreground/90 space-y-3">
                          {message.content.split("\n\n").map((paragraph, idx) => (
                            <div key={idx}>
                              {paragraph.startsWith("**") ? (
                                <p className="font-semibold text-foreground">{paragraph.replace(/\*\*/g, "")}</p>
                              ) : (
                                <div className="space-y-1">
                                  {paragraph.split("\n").map((line, lineIdx) => (
                                    <p key={lineIdx} className="flex items-start gap-2">
                                      {line.startsWith("•") && (
                                        <>
                                          <span className="text-primary mt-0.5">•</span>
                                          <span>{line.slice(2)}</span>
                                        </>
                                      )}
                                      {!line.startsWith("•") && line}
                                    </p>
                                  ))}
                                </div>
                              )}
                            </div>
                          ))}
                        </div>
                      )}

                      {message.codeReferences && message.codeReferences.length > 0 && (
                        <div className="mt-3 flex flex-wrap gap-1.5">
                          {message.codeReferences.map((ref, idx) => (
                            <button
                              key={idx}
                              className="inline-flex items-center gap-1.5 px-2 py-1 bg-muted/50 hover:bg-primary/10 hover:text-primary rounded text-xs font-mono text-muted-foreground transition-all"
                            >
                              <FileCode className="w-3 h-3" />
                              {ref.file}:{ref.line}
                            </button>
                          ))}
                        </div>
                      )}

                      {message.metadata && !message.isStreaming && (
                        <div className="flex items-center gap-3 pt-2 text-[11px] text-muted-foreground/70">
                          {message.metadata.toolCalls && (
                            <span className="flex items-center gap-1">
                              <Wrench className="w-3 h-3" />
                              {message.metadata.toolCalls}
                            </span>
                          )}
                          {message.metadata.thinkingSteps && (
                            <span className="flex items-center gap-1">
                              <Brain className="w-3 h-3" />
                              {message.metadata.thinkingSteps}
                            </span>
                          )}
                          {message.metadata.duration && (
                            <span className="flex items-center gap-1">
                              <Clock className="w-3 h-3" />
                              {message.metadata.duration}
                            </span>
                          )}
                          <button className="ml-auto opacity-0 group-hover:opacity-100 transition-opacity hover:text-foreground p-1 -m-1">
                            <Copy className="w-3 h-3" />
                          </button>
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="flex justify-end">
                      <div className="max-w-[85%] border border-border rounded-lg px-3 py-2.5 bg-muted/30">
                        <div className="text-[13px] text-foreground space-y-1">
                          {message.content.split("\n").map((line, idx) => (
                            <p key={idx} className="flex items-start gap-2">
                              {line.startsWith("•") && (
                                <>
                                  <span className="text-muted-foreground mt-0.5">•</span>
                                  <span>{line.slice(2)}</span>
                                </>
                              )}
                              {!line.startsWith("•") && line}
                            </p>
                          ))}
                        </div>
                      </div>
                    </div>
                  )}
                </div>
              ))}

              {/* Message Queue Section */}
              {messageQueue.length > 0 && (
                <div className="mt-6 space-y-2">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <div className="h-px flex-1 bg-border" />
                    <span className="flex items-center gap-1.5 px-2">
                      <Clock className="w-3 h-3" />
                      {messageQueue.length} queued
                    </span>
                    <div className="h-px flex-1 bg-border" />
                  </div>

                  <DndContext
                    sensors={sensors}
                    collisionDetection={closestCenter}
                    onDragEnd={handleDragEnd}
                  >
                    <SortableContext
                      items={messageQueue.map((m) => m.id)}
                      strategy={verticalListSortingStrategy}
                    >
                      {messageQueue.map((message) => (
                        <SortableQueuedMessage
                          key={message.id}
                          message={message}
                          isEditing={editingMessageId === message.id}
                          editingContent={editingContent}
                          editingAttachments={editingAttachments}
                          onStartEdit={handleStartEdit}
                          onSaveEdit={handleSaveEdit}
                          onCancelEdit={handleCancelEdit}
                          onEditContentChange={setEditingContent}
                          onEditAttachmentsChange={setEditingAttachments}
                          onCancelMessage={handleCancelMessage}
                        />
                      ))}
                    </SortableContext>
                  </DndContext>
                </div>
              )}
            </div>
          </div>

          <div className="relative z-10 -mt-16 pt-8 bg-gradient-to-t from-background via-background to-transparent pointer-events-none">
            <div className="pointer-events-auto">
              {/* Esc hint toast */}
              {escHintVisible && (
                <div className="flex justify-center pb-2">
                  <div className="flex items-center gap-2 px-3 py-2 bg-status-warning/10 border border-status-warning/30 rounded-lg text-xs text-status-warning shadow-lg animate-in fade-in slide-in-from-bottom-2 duration-200">
                    <Keyboard className="w-3.5 h-3.5" />
                    <span>Press <kbd className="px-1.5 py-0.5 bg-status-warning/20 rounded text-[10px] font-mono font-medium">Esc</kbd> again to cancel</span>
                    <div className="w-12 h-1 bg-status-warning/20 rounded-full overflow-hidden">
                      <div 
                        className="h-full bg-status-warning rounded-full"
                        style={{
                          animation: `shrink ${ESC_TIMEOUT_MS}ms linear forwards`,
                        }}
                      />
                    </div>
                  </div>
                </div>
              )}

              {showScrollButton && !editingMessageId && !escHintVisible && (
                <div className="flex justify-center pb-2">
                  <button
                    onClick={scrollToBottom}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-card border border-border rounded-full text-xs text-muted-foreground hover:text-foreground hover:border-primary/50 transition-all shadow-sm hover:shadow-md"
                  >
                    <ArrowDown className="w-3 h-3" />
                    Scroll to bottom
                  </button>
                </div>
              )}

              {/* Hide bottom input when editing a queued message or when showing inline editor */}
              {!editingMessageId && agentRunningStatus !== "cancelling" && agentRunningStatus !== "resuming" && (
                <div className="px-4 pb-4">
                  <div className="max-w-3xl mx-auto">
                    <MessageEditor
                      value={input}
                      onChange={setInput}
                      attachments={attachments}
                      onAttachmentsChange={setAttachments}
                      onSubmit={() => {
                        if (input.trim()) {
                          // Add to history
                          setMessageHistory(prev => [...prev, input.trim()])
                        }
                        console.log("Send:", input, attachments)
                        setInput("")
                        setAttachments([])
                      }}
                      messageHistory={messageHistory}
                      onCommand={(commandId) => {
                        console.log("Command executed:", commandId)
                        // Handle commands
                        switch (commandId) {
                          case "clear":
                            // Would clear conversation
                            console.log("Clear conversation")
                            break
                          case "new":
                            handleAddTab()
                            break
                          case "settings":
                            // Would open settings
                            console.log("Open settings")
                            break
                          default:
                            console.log("Unknown command:", commandId)
                        }
                      }}
                    />
                  </div>
                </div>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  )
}

interface SortableQueuedMessageProps {
  message: Message
  isEditing: boolean
  editingContent: string
  editingAttachments: MessageAttachment[]
  onStartEdit: (message: Message) => void
  onSaveEdit: () => void
  onCancelEdit: () => void
  onEditContentChange: (value: string) => void
  onEditAttachmentsChange: (attachments: MessageAttachment[]) => void
  onCancelMessage: (id: string) => void
}

function SortableQueuedMessage({
  message,
  isEditing,
  editingContent,
  editingAttachments,
  onStartEdit,
  onSaveEdit,
  onCancelEdit,
  onEditContentChange,
  onEditAttachmentsChange,
  onCancelMessage,
}: SortableQueuedMessageProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: message.id,
    disabled: isEditing,
  })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  }

  if (isEditing) {
    return (
      <div
        ref={setNodeRef}
        style={style}
        className="transition-all duration-200 ease-out"
      >
        <MessageEditor
          value={editingContent}
          onChange={onEditContentChange}
          attachments={editingAttachments}
          onAttachmentsChange={onEditAttachmentsChange}
          onSubmit={onSaveEdit}
          onCancel={onCancelEdit}
          placeholder="Edit message..."
          autoFocus
        />
      </div>
    )
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(
        "group flex justify-end transition-all duration-200",
        isDragging && "z-50 opacity-90",
      )}
    >
      <div
        className={cn(
          "relative max-w-[85%] rounded-lg px-3 py-2.5 transition-all duration-200",
          "border border-dashed border-border bg-muted/20 opacity-60 hover:opacity-80",
          isDragging && "shadow-lg border-primary/30 opacity-100 bg-background",
        )}
        onDoubleClick={() => onStartEdit(message)}
        {...attributes}
        {...listeners}
      >
        {/* Cancel button - hidden during dragging */}
        {!isDragging && (
          <button
            onClick={(e) => {
              e.stopPropagation()
              onCancelMessage(message.id)
            }}
            onPointerDown={(e) => e.stopPropagation()}
            className="absolute -top-1.5 -right-1.5 p-1 bg-background border border-border rounded-full shadow-sm opacity-0 group-hover:opacity-100 transition-opacity hover:bg-destructive hover:border-destructive hover:text-destructive-foreground"
          >
            <X className="w-3 h-3" />
          </button>
        )}

        {/* Attachment indicator */}
        {message.attachments && message.attachments.length > 0 && (
          <div className="flex items-center gap-1 mb-1 text-[10px] text-muted-foreground">
            <Paperclip className="w-3 h-3" />
            {message.attachments.length} file{message.attachments.length > 1 ? "s" : ""}
          </div>
        )}

        {/* Message content */}
        <div className="text-[13px] text-foreground/80 line-clamp-2 cursor-grab active:cursor-grabbing">
          {message.content}
        </div>
      </div>
    </div>
  )
}

interface AllFilesDiffViewerProps {
  files: DiffFileData[]
  activeFileId?: string
  diffStyle: "split" | "unified"
  onStyleChange: (style: "split" | "unified") => void
}

function AllFilesDiffViewer({ files, activeFileId, diffStyle, onStyleChange }: AllFilesDiffViewerProps) {
  const fileRefs = useRef<Record<string, HTMLDivElement | null>>({})
  const prevActiveFileIdRef = useRef<string | undefined>(undefined)
  const [collapsedFiles, setCollapsedFiles] = useState<Set<string>>(new Set())
  const shouldForceExpandActiveFile = activeFileId !== undefined && activeFileId !== prevActiveFileIdRef.current

  const toggleCollapse = (fileId: string) => {
    setCollapsedFiles(prev => {
      const next = new Set(prev)
      if (next.has(fileId)) {
        next.delete(fileId)
      } else {
        next.add(fileId)
      }
      return next
    })
  }

  useEffect(() => {
    if (activeFileId && activeFileId !== prevActiveFileIdRef.current && fileRefs.current[activeFileId]) {
      fileRefs.current[activeFileId]?.scrollIntoView({ behavior: "smooth", block: "start" })
    }
    prevActiveFileIdRef.current = activeFileId
  }, [activeFileId])

  const getStatusColor = (status: string) => {
    switch (status) {
      case "modified": return "text-status-warning"
      case "added": return "text-status-success"
      case "deleted": return "text-status-error"
      case "renamed": return "text-status-info"
      default: return "text-muted-foreground"
    }
  }

  const getStatusLabel = (status: string) => {
    switch (status) {
      case "modified": return "M"
      case "added": return "A"
      case "deleted": return "D"
      case "renamed": return "R"
      default: return "?"
    }
  }

  const totalAdditions = files.reduce((sum, f) => sum + (f.file.additions ?? 0), 0)
  const totalDeletions = files.reduce((sum, f) => sum + (f.file.deletions ?? 0), 0)

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-background">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-4 py-2 bg-muted/50 border-b border-border text-xs">
        <span className="text-foreground font-medium">{files.length} files changed</span>
        <span className="text-muted-foreground">
          {totalAdditions > 0 && <span className="text-status-success">+{totalAdditions}</span>}
          {totalAdditions > 0 && totalDeletions > 0 && <span className="mx-1">/</span>}
          {totalDeletions > 0 && <span className="text-status-error">-{totalDeletions}</span>}
        </span>
        <div className="ml-auto flex items-center gap-0.5 p-0.5 bg-muted rounded">
          <button
            onClick={() => onStyleChange("split")}
            className={cn(
              "p-1 rounded transition-colors",
              diffStyle === "split"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            title="Split view"
          >
            <Columns2 className="w-3.5 h-3.5" />
          </button>
          <button
            onClick={() => onStyleChange("unified")}
            className={cn(
              "p-1 rounded transition-colors",
              diffStyle === "unified"
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
            title="Unified view"
          >
            <AlignJustify className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {/* Scrollable diff content */}
      <div className="flex-1 overflow-auto">
	        {files.map((fileData) => {
	          const isCollapsed =
              collapsedFiles.has(fileData.file.id) &&
              !(shouldForceExpandActiveFile && fileData.file.id === activeFileId)
	          return (
            <div
              key={fileData.file.id}
              ref={(el) => { fileRefs.current[fileData.file.id] = el }}
              className="border-b border-border last:border-b-0"
            >
              {/* File header */}
              <button
                onClick={() => toggleCollapse(fileData.file.id)}
                className="sticky top-0 z-[5] w-full flex items-center gap-2 px-4 py-2 bg-muted/80 backdrop-blur-sm border-b border-border/50 text-xs hover:bg-muted transition-colors text-left"
              >
                {isCollapsed ? (
                  <ChevronRight className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                ) : (
                  <ChevronDown className="w-3 h-3 text-muted-foreground flex-shrink-0" />
                )}
                <span className={cn("font-mono font-semibold", getStatusColor(fileData.file.status))}>
                  {getStatusLabel(fileData.file.status)}
                </span>
                <span className="font-mono text-foreground">{fileData.file.path}</span>
                {fileData.file.additions !== undefined && fileData.file.additions > 0 && (
                  <span className="text-status-success">+{fileData.file.additions}</span>
                )}
                {fileData.file.deletions !== undefined && fileData.file.deletions > 0 && (
                  <span className="text-status-error">-{fileData.file.deletions}</span>
                )}
              </button>
              {!isCollapsed && (
                <MultiFileDiff
                  oldFile={fileData.oldFile}
                  newFile={fileData.newFile}
                  options={{
                    theme: { dark: "pierre-dark", light: "pierre-light" },
                    diffStyle: diffStyle,
                    diffIndicators: "bars",
                    hunkSeparators: "line-info",
                    lineDiffType: "word-alt",
                    enableLineSelection: true,
                  }}
                />
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
