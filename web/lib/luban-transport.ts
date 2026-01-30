"use client"

import { useEffect, useRef, useState } from "react"

import type { ClientAction, ServerEvent, WsClientMessage, WsServerMessage } from "./luban-api"
import { isMockMode } from "./luban-mode"
import { mockDispatchAction, mockRequest } from "./mock/mock-runtime"

const PROTOCOL_VERSION = 1
const MAX_PENDING_ACTIONS = 128
const HEARTBEAT_INTERVAL_MS = 20_000
const RECONNECT_BASE_DELAY_MS = 200
const RECONNECT_MAX_DELAY_MS = 5_000

function randomRequestId(): string {
  return `req_${Math.random().toString(16).slice(2)}_${Date.now().toString(16)}`
}

function wsUrl(path: string): string {
  const url = new URL(path, window.location.href)
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:"
  return url.toString()
}

type PendingResponse = {
  resolve: (value: unknown) => void
  reject: (err: Error) => void
}

type QueuedAction = {
  action: ClientAction
  requestId: string
}

export function useLubanTransport(args: {
  onEvent: (event: ServerEvent) => void
  onError: (message: string) => void
}): {
  wsConnected: boolean
  sendAction: (action: ClientAction, requestId?: string) => void
  request: <T>(action: ClientAction) => Promise<T>
} {
  const [wsConnected, setWsConnected] = useState(false)

  const wsRef = useRef<WebSocket | null>(null)
  const pendingActionsRef = useRef<QueuedAction[]>([])
  const pendingResponsesRef = useRef<Map<string, PendingResponse>>(new Map())
  const handlersRef = useRef({ onEvent: args.onEvent, onError: args.onError })
  const lastSeenRevRef = useRef<number | null>(null)

  useEffect(() => {
    handlersRef.current = { onEvent: args.onEvent, onError: args.onError }
  }, [args.onEvent, args.onError])

  function sendAction(action: ClientAction, requestId?: string) {
    if (isMockMode()) {
      try {
        mockDispatchAction({ action, onEvent: handlersRef.current.onEvent })
      } catch (err) {
        handlersRef.current.onError(err instanceof Error ? err.message : String(err))
      }
      return
    }

    const resolvedRequestId = requestId ?? randomRequestId()
    const ws = wsRef.current
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      const queue = pendingActionsRef.current
      if (queue.length < MAX_PENDING_ACTIONS) {
        queue.push({ action, requestId: resolvedRequestId })
      } else {
        queue.shift()
        queue.push({ action, requestId: resolvedRequestId })
      }
      return
    }
    const msg: WsClientMessage = { type: "action", request_id: resolvedRequestId, action }
    ws.send(JSON.stringify(msg))
  }

  function request<T>(action: ClientAction): Promise<T> {
    if (isMockMode()) {
      return mockRequest<T>(action)
    }

    const requestId = randomRequestId()
    return new Promise<T>((resolve, reject) => {
      pendingResponsesRef.current.set(requestId, {
        resolve: (value) => resolve(value as T),
        reject,
      })
      sendAction(action, requestId)
    })
  }

  useEffect(() => {
    if (isMockMode()) {
      setWsConnected(true)
      const pending = pendingActionsRef.current.splice(0, pendingActionsRef.current.length)
      for (const { action } of pending) {
        try {
          mockDispatchAction({ action, onEvent: handlersRef.current.onEvent })
        } catch (err) {
          handlersRef.current.onError(err instanceof Error ? err.message : String(err))
        }
      }
      return
    }

    let disposed = false
    let reconnectTimer: number | null = null
    let heartbeatTimer: number | null = null
    let connectAttempt = 0

    function rejectAllPending(reason: string) {
      for (const [, pending] of pendingResponsesRef.current) {
        pending.reject(new Error(reason))
      }
      pendingResponsesRef.current.clear()
    }

    function scheduleReconnect() {
      if (disposed) return
      if (reconnectTimer != null) return
      const exp = Math.min(connectAttempt, 6)
      const base = Math.min(RECONNECT_MAX_DELAY_MS, RECONNECT_BASE_DELAY_MS * 2 ** exp)
      const jitter = Math.floor(Math.random() * 100)
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null
        connect()
      }, base + jitter)
      connectAttempt += 1
    }

    function stopHeartbeat() {
      if (heartbeatTimer != null) {
        window.clearInterval(heartbeatTimer)
        heartbeatTimer = null
      }
    }

    function startHeartbeat(ws: WebSocket) {
      stopHeartbeat()
      heartbeatTimer = window.setInterval(() => {
        if (ws.readyState !== WebSocket.OPEN) return
        const ping: WsClientMessage = { type: "ping" }
        ws.send(JSON.stringify(ping))
      }, HEARTBEAT_INTERVAL_MS)
    }

    function connect() {
      if (disposed) return

      const ws = new WebSocket(wsUrl("/api/events"))
      wsRef.current = ws

      ws.onopen = () => {
        connectAttempt = 0
        setWsConnected(true)

        const hello: WsClientMessage = {
          type: "hello",
          protocol_version: PROTOCOL_VERSION,
          last_seen_rev: lastSeenRevRef.current,
        }
        ws.send(JSON.stringify(hello))

        startHeartbeat(ws)

        const pending = pendingActionsRef.current.splice(0, pendingActionsRef.current.length)
        for (const queued of pending) {
          const msg: WsClientMessage = { type: "action", request_id: queued.requestId, action: queued.action }
          ws.send(JSON.stringify(msg))
        }
      }

      ws.onmessage = (ev) => {
        if (typeof ev.data !== "string") return

        let msg: WsServerMessage
        try {
          msg = JSON.parse(ev.data) as WsServerMessage
        } catch {
          handlersRef.current.onError("Invalid server message")
          return
        }

        if (msg.type === "event") {
          lastSeenRevRef.current = msg.rev
          const event = msg.event

          if (
            event.type === "project_path_picked" ||
            event.type === "add_project_and_open_ready" ||
            event.type === "task_preview_ready" ||
            event.type === "task_executed" ||
            event.type === "feedback_submitted" ||
            event.type === "codex_check_ready" ||
            event.type === "codex_config_tree_ready" ||
            event.type === "codex_config_list_dir_ready" ||
            event.type === "codex_config_file_ready" ||
            event.type === "codex_config_file_saved" ||
            event.type === "amp_check_ready" ||
            event.type === "amp_config_tree_ready" ||
            event.type === "amp_config_list_dir_ready" ||
            event.type === "amp_config_file_ready" ||
            event.type === "amp_config_file_saved" ||
            event.type === "claude_check_ready" ||
            event.type === "claude_config_tree_ready" ||
            event.type === "claude_config_list_dir_ready" ||
            event.type === "claude_config_file_ready" ||
            event.type === "claude_config_file_saved"
          ) {
            const pending = pendingResponsesRef.current.get(event.request_id)
            if (pending) {
              pendingResponsesRef.current.delete(event.request_id)
              if (event.type === "project_path_picked") pending.resolve(event.path)
              if (event.type === "add_project_and_open_ready")
                pending.resolve({ projectId: event.project_id, workdirId: event.workdir_id })
              if (event.type === "task_preview_ready") pending.resolve(event.draft)
              if (event.type === "task_executed") pending.resolve(event.result)
              if (event.type === "feedback_submitted") pending.resolve(event.result)
              if (event.type === "codex_check_ready") pending.resolve({ ok: event.ok, message: event.message })
              if (event.type === "codex_config_tree_ready") pending.resolve(event.tree)
              if (event.type === "codex_config_list_dir_ready")
                pending.resolve({ path: event.path, entries: event.entries })
              if (event.type === "codex_config_file_ready") pending.resolve(event.contents)
              if (event.type === "codex_config_file_saved") pending.resolve(null)
              if (event.type === "amp_check_ready") pending.resolve({ ok: event.ok, message: event.message })
              if (event.type === "amp_config_tree_ready") pending.resolve(event.tree)
              if (event.type === "amp_config_list_dir_ready")
                pending.resolve({ path: event.path, entries: event.entries })
              if (event.type === "amp_config_file_ready") pending.resolve(event.contents)
              if (event.type === "amp_config_file_saved") pending.resolve(null)
              if (event.type === "claude_check_ready") pending.resolve({ ok: event.ok, message: event.message })
              if (event.type === "claude_config_tree_ready") pending.resolve(event.tree)
              if (event.type === "claude_config_list_dir_ready")
                pending.resolve({ path: event.path, entries: event.entries })
              if (event.type === "claude_config_file_ready") pending.resolve(event.contents)
              if (event.type === "claude_config_file_saved") pending.resolve(null)
            }
            return
          }

          handlersRef.current.onEvent(event)
          return
        }

        if (msg.type === "error") {
          if (msg.request_id) {
            const pending = pendingResponsesRef.current.get(msg.request_id)
            if (pending) {
              pendingResponsesRef.current.delete(msg.request_id)
              pending.reject(new Error(msg.message))
              return
            }
          }
          handlersRef.current.onError(msg.message)
        }
      }

      ws.onerror = () => {}

      ws.onclose = () => {
        if (wsRef.current !== ws) return
        stopHeartbeat()
        setWsConnected(false)
        rejectAllPending("disconnected")
        scheduleReconnect()
      }
    }

    connect()

    return () => {
      disposed = true
      if (reconnectTimer != null) window.clearTimeout(reconnectTimer)
      stopHeartbeat()
      rejectAllPending("disconnected")
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [])

  return { wsConnected, sendAction, request }
}
