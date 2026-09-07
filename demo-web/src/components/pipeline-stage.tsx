import { Check, Circle, Loader } from "lucide-react"
import type { DemoStageState } from "@/data/demo"
import { cn } from "@/lib/utils"

export function PipelineStage({ index, label, state }: { index: number; label: string; state: DemoStageState }) {
  return (
    <div className={cn("flex items-center gap-3 rounded-sm border px-3 py-2.5 transition-colors", state === "complete" && "border-pass/25 bg-pass/[0.06]", state === "running" && "border-primary/40 bg-primary/[0.08]", state === "waiting" && "border-border bg-elevated")}>
      <span className="flex h-5 w-5 items-center justify-center">{state === "complete" && <Check className="h-4 w-4 text-pass" aria-hidden />}{state === "running" && <Loader className="h-4 w-4 animate-spin text-primary" aria-hidden />}{state === "waiting" && <Circle className="h-3 w-3 text-border" aria-hidden />}</span>
      <span className="font-mono text-xs text-muted-foreground">{String(index + 1).padStart(2, "0")}</span>
      <span className={cn("text-sm", state === "waiting" ? "text-muted-foreground" : "text-foreground")}>{label}</span>
      <span className={cn("ml-auto font-mono text-[10px] uppercase tracking-wider", state === "complete" && "text-pass", state === "running" && "text-primary", state === "waiting" && "text-muted-foreground/60")}>{state}</span>
    </div>
  )
}
