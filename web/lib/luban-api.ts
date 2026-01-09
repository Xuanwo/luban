export type ProjectId = number
export type WorkspaceId = number
export type WorkspaceThreadId = number

export type WorkspaceStatus = "active" | "archived"

export type AppSnapshot = {
  rev: number
  projects: ProjectSnapshot[]
}

export type ProjectSnapshot = {
  id: ProjectId
  name: string
  slug: string
  path: string
  expanded: boolean
  create_workspace_status: OperationStatus
  workspaces: WorkspaceSnapshot[]
}

export type WorkspaceSnapshot = {
  id: WorkspaceId
  short_id: string
  workspace_name: string
  branch_name: string
  worktree_path: string
  status: WorkspaceStatus
  agent_run_status: OperationStatus
  has_unread_completion: boolean
  pull_request: PullRequestSnapshot | null
}

export type PullRequestState = "open" | "closed" | "merged"

export type PullRequestCiState = "pending" | "success" | "failure"

export type PullRequestSnapshot = {
  number: number
  is_draft: boolean
  state: PullRequestState
  ci_state: PullRequestCiState | null
  merge_ready: boolean
}

export type ThreadsSnapshot = {
  rev: number
  workspace_id: WorkspaceId
  threads: ThreadMeta[]
}

export type ThreadMeta = {
  thread_id: WorkspaceThreadId
  remote_thread_id: string | null
  title: string
  updated_at_unix_seconds: number
}

export type AttachmentKind = "image" | "text" | "file"

export type AttachmentRef = {
  id: string
  kind: AttachmentKind
  name: string
  extension: string
  mime: string | null
  byte_len: number
}

export type ConversationSnapshot = {
  rev: number
  workspace_id: WorkspaceId
  thread_id: WorkspaceThreadId
  agent_model_id: string
  thinking_effort: ThinkingEffort
  run_status: OperationStatus
  entries: ConversationEntry[]
  in_progress_items: AgentItem[]
  remote_thread_id: string | null
  title: string
}

export type OperationStatus = "idle" | "running"

export type ThinkingEffort = "low" | "medium" | "high" | "xhigh"

export type AgentItemKind =
  | "agent_message"
  | "reasoning"
  | "command_execution"
  | "file_change"
  | "mcp_tool_call"
  | "web_search"
  | "todo_list"
  | "error"

export type AgentItem = {
  id: string
  kind: AgentItemKind
  payload: unknown
}

export type ConversationEntry =
  | { type: "user_message"; text: string; attachments: AttachmentRef[] }
  | { type: "agent_item"; id: string; kind: AgentItemKind; payload: unknown }
  | { type: "turn_usage"; usage_json: unknown | null }
  | { type: "turn_duration"; duration_ms: number }
  | { type: "turn_canceled" }
  | { type: "turn_error"; message: string }

export type ClientAction =
  | { type: "pick_project_path" }
  | { type: "add_project"; path: string }
  | { type: "delete_project"; project_id: ProjectId }
  | { type: "toggle_project_expanded"; project_id: ProjectId }
  | { type: "create_workspace"; project_id: ProjectId }
  | { type: "open_workspace"; workspace_id: WorkspaceId }
  | { type: "open_workspace_pull_request"; workspace_id: WorkspaceId }
  | { type: "open_workspace_pull_request_failed_action"; workspace_id: WorkspaceId }
  | { type: "archive_workspace"; workspace_id: WorkspaceId }
  | {
      type: "send_agent_message"
      workspace_id: WorkspaceId
      thread_id: WorkspaceThreadId
      text: string
      attachments: AttachmentRef[]
    }
  | { type: "cancel_agent_turn"; workspace_id: WorkspaceId; thread_id: WorkspaceThreadId }
  | { type: "create_workspace_thread"; workspace_id: WorkspaceId }
  | { type: "activate_workspace_thread"; workspace_id: WorkspaceId; thread_id: WorkspaceThreadId }
  | { type: "close_workspace_thread_tab"; workspace_id: WorkspaceId; thread_id: WorkspaceThreadId }
  | { type: "restore_workspace_thread_tab"; workspace_id: WorkspaceId; thread_id: WorkspaceThreadId }
  | {
      type: "reorder_workspace_thread_tab"
      workspace_id: WorkspaceId
      thread_id: WorkspaceThreadId
      to_index: number
    }

export type ServerEvent =
  | { type: "app_changed"; rev: number; snapshot: AppSnapshot }
  | { type: "workspace_threads_changed"; workspace_id: WorkspaceId; threads: ThreadMeta[] }
  | { type: "conversation_changed"; snapshot: ConversationSnapshot }
  | { type: "toast"; message: string }

export type WsClientMessage =
  | { type: "hello"; protocol_version: number; last_seen_rev: number | null }
  | { type: "action"; request_id: string; action: ClientAction }
  | { type: "ping" }

export type WsServerMessage =
  | { type: "hello"; protocol_version: number; current_rev: number }
  | { type: "ack"; request_id: string; rev: number }
  | { type: "event"; rev: number; event: ServerEvent }
  | { type: "error"; request_id: string | null; message: string }
  | { type: "pong" }
