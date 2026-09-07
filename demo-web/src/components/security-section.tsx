import { Fragment } from "react"
import { ChevronRight, Lock, ShieldCheck } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { pqcFlow, pqcProperties } from "@/data/project"

export function SecuritySection() {
  return (
    <section id="security" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading index="06 / Security" title="Experimental post-quantum artifact protection" description="M5 is an optional, separate file-protection feature. It can read a committed artifact, but it does not automatically protect M3 output or write inside M4-managed namespaces." />
        <div className="rounded-md border border-border bg-card p-5"><div className="mb-4 flex items-center gap-2"><Lock className="h-4 w-4 text-primary" aria-hidden /><span className="font-mono text-xs uppercase tracking-widest text-primary/80">Protection scheme</span></div><ol className="flex flex-wrap items-stretch gap-y-3">{pqcFlow.map((node, index) => <Fragment key={node.label}><li className="flex min-w-[130px] flex-1 flex-col gap-1 rounded-sm border border-border bg-elevated px-3 py-2.5"><span className="font-mono text-xs font-medium text-foreground">{node.label}</span><span className="text-xs text-muted-foreground">{node.note}</span></li>{index < pqcFlow.length - 1 ? <ChevronRight className="mx-1 h-4 w-4 shrink-0 self-center text-border" aria-hidden /> : null}</Fragment>)}</ol></div>
        <div className="mt-4 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">{pqcProperties.map((property) => <div key={property.label} className="rounded-md border border-border bg-card p-4"><div className="flex items-center gap-1.5"><ShieldCheck className="h-3.5 w-3.5 text-pass" aria-hidden /><span className="font-mono text-xs uppercase tracking-wide text-muted-foreground">{property.label}</span></div><div className="mt-1.5 font-mono text-sm text-foreground">{property.value}</div></div>)}</div>
      </div>
    </section>
  )
}
