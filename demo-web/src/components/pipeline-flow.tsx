import { Fragment } from "react"
import { ChevronRight } from "lucide-react"
import { pipelineStages } from "@/data/project"

export function PipelineFlow() {
  return (
    <div className="rounded-md border border-border bg-card/60 p-4">
      <div className="mb-3 flex items-center gap-2">
        <span className="h-1.5 w-1.5 rounded-full bg-primary" aria-hidden />
        <span className="font-mono text-xs uppercase tracking-widest text-muted-foreground">Conceptual pipeline</span>
      </div>
      <ol className="flex flex-wrap items-center gap-y-2">
        {pipelineStages.map((stage, index) => (
          <Fragment key={stage.key}>
            <li className="flex items-center gap-2 rounded-sm border border-border bg-elevated px-2.5 py-1.5">
              <span className="font-mono text-xs text-muted-foreground">{String(index).padStart(2, "0")}</span>
              <span className="font-mono text-xs font-medium text-foreground">{stage.label}</span>
              {"optional" in stage ? <span className="text-[10px] uppercase tracking-wide text-primary">optional</span> : null}
            </li>
            {index < pipelineStages.length - 1 ? <ChevronRight className="mx-0.5 h-3.5 w-3.5 shrink-0 text-border" aria-hidden /> : null}
          </Fragment>
        ))}
      </ol>
      <p className="mt-3 text-xs leading-relaxed text-muted-foreground">M4 is an additional recoverable conversion mode. M5 is separate, optional artifact protection.</p>
    </div>
  )
}
