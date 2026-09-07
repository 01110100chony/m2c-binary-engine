import type { Status } from "@/data/project"
import { cn } from "@/lib/utils"

export function MetricCard({ value, label, sub, status }: { value: string; label: string; sub?: string; status?: Status }) {
  return (
    <div className="flex flex-col gap-1 rounded-md border border-border bg-card p-5">
      <div className={cn("font-mono text-2xl font-semibold tracking-tight tabular-nums md:text-[28px]", status === "PASS" ? "text-pass" : "text-foreground")}>{value}</div>
      <div className="text-sm font-medium text-foreground/90">{label}</div>
      {sub ? <div className="font-mono text-xs text-muted-foreground">{sub}</div> : null}
    </div>
  )
}
