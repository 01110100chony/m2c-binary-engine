import { Check } from "lucide-react"
import type { TerminalLine } from "@/data/demo"
import { cn } from "@/lib/utils"

export function TerminalPanel({ lines, revealCount, className }: { lines: readonly TerminalLine[]; revealCount?: number; className?: string }) {
  const count = revealCount ?? lines.length
  return (
    <div className={cn("overflow-hidden rounded-md border border-border bg-[oklch(0.13_0.004_240)]", className)}>
      <div className="flex items-center gap-1.5 border-b border-border px-3 py-2"><span className="h-2 w-2 rounded-full bg-error/60" aria-hidden /><span className="h-2 w-2 rounded-full bg-warn/60" aria-hidden /><span className="h-2 w-2 rounded-full bg-pass/60" aria-hidden /><span className="ml-2 font-mono text-xs text-muted-foreground">m2c — simulated</span></div>
      <div className="min-h-[132px] p-3 font-mono text-xs leading-relaxed">
        {lines.slice(0, count).map((line, index) => <div key={`${line.text}-${index}`} className={cn("flex items-start gap-1.5 whitespace-pre-wrap break-words", line.type === "cmd" && "text-foreground/80", line.type === "ok" && "text-pass")}>{line.type === "ok" ? <Check className="mt-0.5 h-3 w-3 shrink-0" aria-hidden /> : null}<span>{line.text}</span></div>)}
      </div>
    </div>
  )
}
