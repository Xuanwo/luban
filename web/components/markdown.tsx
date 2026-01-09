"use client"

import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"

import { cn } from "@/lib/utils"

function normalizeMarkdown(input: string): string {
  return input
    .split("\n")
    .map((line) => (line.startsWith("• ") ? `- ${line.slice(2)}` : line))
    .join("\n")
}

export function Markdown({ content, className }: { content: string; className?: string }) {
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
          a: ({ className, ...props }) => (
            <a className={cn("text-primary underline underline-offset-2", className)} {...props} />
          ),
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
          pre: ({ className, ...props }) => (
            <pre
              className={cn(
                "my-2 p-2 overflow-x-auto rounded border border-border bg-muted/30 text-[12px]",
                className,
              )}
              {...props}
            />
          ),
        }}
      >
        {normalizeMarkdown(content)}
      </ReactMarkdown>
    </div>
  )
}
