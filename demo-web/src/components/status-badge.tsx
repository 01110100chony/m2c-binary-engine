import type { Status } from "@/data/project"
import { cn } from "@/lib/utils"

const styles: Record<Status, string> = {
  PASS: "border-pass/30 bg-pass/10 text-pass",
  WARN: "border-warn/30 bg-warn/10 text-warn",
  FAIL: "border-error/30 bg-error/10 text-error",
}
const dots: Record<Status, string> = { PASS: "bg-pass", WARN: "bg-warn", FAIL: "bg-error" }

export function StatusBadge({ status, label, className }: { status: Status; label?: string; className?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-1.5 rounded-sm border px-2 py-0.5 font-mono text-xs font-medium tracking-wide", styles[status], className)}>
      <span className={cn("h-1.5 w-1.5 rounded-full", dots[status])} aria-hidden />
      {label ?? status}
    </span>
  )
}
