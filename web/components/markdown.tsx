"use client"

import { isValidElement, memo } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"

import { cn } from "@/lib/utils"
import { openExternalUrl } from "@/lib/open-external-url"
import { MermaidDiagram } from "@/components/mermaid-diagram"

function normalizeMarkdown(input: string): string {
  return input
    .split("\n")
    .map((line) => (line.startsWith("• ") ? `- ${line.slice(2)}` : line))
    .join("\n")
}

export const Markdown = memo(
  function Markdown({
    content,
    className,
    enableMermaid,
  }: {
    content: string
    className?: string
    enableMermaid?: boolean
  }) {
    return (
      <div className={cn("text-[13px] leading-relaxed text-foreground/90 break-words overflow-hidden", className)}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          components={{
          p: (props) => <p className="my-2 first:mt-0 last:mb-0" {...props} />,
          h1: (props) => <h1 className="mt-4 mb-2 text-base font-semibold text-foreground" {...props} />,
          h2: (props) => <h2 className="mt-4 mb-2 text-[15px] font-semibold text-foreground" {...props} />,
          h3: (props) => <h3 className="mt-3 mb-1.5 text-sm font-semibold text-foreground" {...props} />,
          ul: (props) => <ul className="my-2 pl-5 list-disc space-y-1" {...props} />,
          ol: (props) => <ol className="my-2 pl-5 list-decimal space-y-1" {...props} />,
          li: (props) => <li className="text-[13px]" {...props} />,
          blockquote: (props) => (
            <blockquote className="my-2 border-l-2 border-border pl-3 text-muted-foreground" {...props} />
          ),
          a: ({ className, href, onClick, ...props }) => (
            <a
              className={cn("text-primary underline underline-offset-2", className)}
              href={href}
              target="_blank"
              rel="noreferrer noopener"
              onClick={(ev) => {
                onClick?.(ev)
                if (ev.defaultPrevented) return
                if (!href) return
                ev.preventDefault()
                void openExternalUrl(String(href))
              }}
              {...props}
            />
          ),
          img: ({ className, src, alt, ...props }) => {
            if (!src) return null
            return (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                className={cn("my-2 max-w-full h-auto rounded-md border border-border/50", className)}
                src={String(src)}
                alt={alt ?? ""}
                loading="lazy"
                {...props}
              />
            )
          },
          code: ({ className, children, ...props }) => {
            const isBlock = typeof className === "string" && className.includes("language-")
            if (isBlock) {
              return (
                <code className={cn("font-mono text-[12px]", className)} {...props}>
                  {children}
                </code>
              )
            }
            return (
              <code
                className={cn(
                  "font-mono text-[12px] px-1 py-0.5 rounded bg-muted/50 border border-border/60 break-all whitespace-pre-wrap",
                  className,
                )}
                {...props}
              >
                {children}
              </code>
            )
          },
          pre: ({ className, children, ...props }) => {
            if (!enableMermaid) {
              return (
                <pre
                  className={cn(
                    "my-2 p-2 overflow-x-auto rounded border border-border bg-muted/30 text-[12px]",
                    className,
                  )}
                  {...props}
                >
                  {children}
                </pre>
              )
            }

            const first = Array.isArray(children) ? children[0] : children
            if (isValidElement<{ className?: string; children?: unknown }>(first)) {
              const childClassName = String(first.props?.className ?? "")
              if (childClassName.includes("language-mermaid")) {
                const raw = String(first.props?.children ?? "")
                return <MermaidDiagram code={raw} />
              }
            }

            return (
              <pre
                className={cn("my-2 p-2 overflow-x-auto rounded border border-border bg-muted/30 text-[12px]", className)}
                {...props}
              >
                {children}
              </pre>
            )
          },
          }}
        >
          {normalizeMarkdown(content)}
        </ReactMarkdown>
      </div>
    )
  },
  (prev, next) =>
    prev.content === next.content && prev.className === next.className && prev.enableMermaid === next.enableMermaid,
)
