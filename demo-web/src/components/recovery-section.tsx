import { Fragment } from "react"
import { ArrowRight, Check } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { outputTree, recoveryProperties, recoverySteps } from "@/data/project"

export function RecoverySection() {
  return (
    <section id="reliability" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading index="05 / Reliability" title="Deterministic recovery" description="M4 is an additional local conversion mode with deterministic parts, immutable commit receipts, and explicit resume validation. Its process-failure guarantees do not claim power-loss or operating-system durability." />
        <div className="grid gap-4 lg:grid-cols-2">
          <div className="rounded-md border border-border bg-card p-5"><span className="font-mono text-xs uppercase tracking-widest text-primary/80">Output layout</span><pre className="mt-3 overflow-x-auto rounded-sm border border-border bg-[oklch(0.14_0.004_240)] p-4 font-mono text-xs leading-relaxed text-muted-foreground"><code>{outputTree}</code></pre><p className="mt-3 font-mono text-xs text-muted-foreground">Deterministic part names · immutable commit receipts</p></div>
          <div className="flex flex-col gap-4">
            <div className="rounded-md border border-border bg-card p-5"><span className="font-mono text-xs uppercase tracking-widest text-primary/80">Resume flow</span><ol className="mt-3 flex flex-wrap items-center gap-x-2 gap-y-2">{recoverySteps.map((step, index) => <Fragment key={step}><li className="rounded-sm border border-border bg-elevated px-2.5 py-1.5 text-xs text-foreground">{step}</li>{index < recoverySteps.length - 1 ? <ArrowRight className="h-3.5 w-3.5 shrink-0 text-border" aria-hidden /> : null}</Fragment>)}</ol></div>
            <div className="rounded-md border border-border bg-card p-5"><span className="font-mono text-xs uppercase tracking-widest text-primary/80">Properties</span><ul className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">{recoveryProperties.map((property) => <li key={property} className="flex items-center gap-2 text-sm text-foreground/90"><Check className="h-4 w-4 shrink-0 text-pass" aria-hidden />{property}</li>)}</ul></div>
          </div>
        </div>
      </div>
    </section>
  )
}
