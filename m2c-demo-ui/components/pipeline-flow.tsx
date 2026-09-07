import { Fragment } from "react"
import { ChevronRight } from "lucide-react"
import { pipelineStages } from "@/lib/data"

export function PipelineFlow() {
  return (
    <div className="rounded-md border border-border bg-card/60 p-4">
      <div className="mb-3 flex items-center gap-2">
        <span className="h-1.5 w-1.5 rounded-full bg-primary" aria-hidden />
        <span className="font-mono text-[11px] uppercase tracking-widest text-muted-foreground">
          Conversion pipeline
        </span>
      </div>
      <ol className="flex flex-wrap items-center gap-y-2">
        {pipelineStages.map((stage, i) => (
          <Fragment key={stage.key}>
            <li className="flex items-center gap-2 rounded-sm border border-border bg-elevated px-2.5 py-1.5">
              <span className="font-mono text-[11px] text-muted-foreground">
                {String(i).padStart(2, "0")}
              </span>
              <span className="font-mono text-xs font-medium text-foreground">{stage.label}</span>
            </li>
            {i < pipelineStages.length - 1 ? (
              <ChevronRight className="mx-0.5 h-3.5 w-3.5 shrink-0 text-border" aria-hidden />
            ) : null}
          </Fragment>
        ))}
      </ol>
    </div>
  )
}
