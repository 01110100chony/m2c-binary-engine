import { cn } from "@/lib/utils"
import type { Status } from "@/lib/data"

export function MetricCard({
  value,
  label,
  sub,
  status,
}: {
  value: string
  label: string
  sub?: string
  status?: Status
}) {
  const isStatus = status === "PASS"

  return (
    <div className="flex flex-col gap-1 rounded-md border border-border bg-card p-5">
      <div
        className={cn(
          "font-mono text-2xl font-semibold tracking-tight tabular-nums md:text-[28px]",
          isStatus ? "text-pass" : "text-foreground",
        )}
      >
        {value}
      </div>
      <div className="text-sm font-medium text-foreground/90">{label}</div>
      {sub ? <div className="font-mono text-xs text-muted-foreground">{sub}</div> : null}
    </div>
  )
}
