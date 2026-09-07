import { cn } from "@/lib/utils"

type Bar = { label: string; value: number }

export function BenchmarkChart({ title, dataset, unit, bars, highlightMin = true }: { title: string; dataset?: string; unit: string; bars: readonly Bar[]; highlightMin?: boolean }) {
  const max = Math.max(...bars.map((bar) => bar.value))
  const minValue = Math.min(...bars.map((bar) => bar.value))
  return (
    <div className="rounded-md border border-border bg-card p-5">
      <div className="mb-1 flex items-baseline justify-between gap-3"><h3 className="text-sm font-medium text-foreground">{title}</h3><span className="font-mono text-xs text-muted-foreground">{unit}</span></div>
      {dataset ? <p className="mb-4 font-mono text-xs text-muted-foreground">{dataset}</p> : <div className="mb-4" />}
      <div className="flex flex-col gap-3">{bars.map((bar) => {
        const width = Math.max(4, (bar.value / max) * 100)
        const isBest = highlightMin && bar.value === minValue
        return <div key={bar.label} className="flex flex-col gap-1.5"><div className="flex items-baseline justify-between gap-3"><span className="font-mono text-xs text-foreground/80">{bar.label}</span><span className={cn("font-mono text-xs tabular-nums", isBest ? "text-primary" : "text-muted-foreground")}>{bar.value.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}</span></div><div className="h-2 w-full overflow-hidden rounded-sm bg-elevated"><div className={cn("h-full rounded-sm transition-[width] duration-500", isBest ? "bg-primary" : "bg-muted-foreground/40")} style={{ width: `${width}%` }} /></div></div>
      })}</div>
    </div>
  )
}
