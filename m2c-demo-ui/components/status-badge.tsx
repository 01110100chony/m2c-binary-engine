import { cn } from "@/lib/utils"
import type { Status } from "@/lib/data"

const styles: Record<Status, string> = {
  PASS: "text-pass border-pass/30 bg-pass/10",
  WARN: "text-warn border-warn/30 bg-warn/10",
  FAIL: "text-error border-error/30 bg-error/10",
}

const dotStyles: Record<Status, string> = {
  PASS: "bg-pass",
  WARN: "bg-warn",
  FAIL: "bg-error",
}

export function StatusBadge({
  status,
  label,
  className,
}: {
  status: Status
  label?: string
  className?: string
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-sm border px-2 py-0.5 font-mono text-[11px] font-medium tracking-wide",
        styles[status],
        className,
      )}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", dotStyles[status])} aria-hidden />
      {label ?? status}
    </span>
  )
}
