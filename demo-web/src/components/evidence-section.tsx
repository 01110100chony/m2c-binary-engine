import { StatusBadge } from "@/components/status-badge"
import { evidence } from "@/data/project"

export function EvidenceSection() {
  return (
    <section aria-label="Verification evidence" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6"><div className="rounded-md border border-border bg-card p-5"><div className="mb-4 flex flex-wrap items-center justify-between gap-2"><span className="font-mono text-xs uppercase tracking-widest text-primary/80">Verification gates</span><span className="font-mono text-xs text-muted-foreground">{evidence.length}/{evidence.length} passing</span></div><ul className="grid grid-cols-1 gap-px overflow-hidden rounded-sm border border-border bg-border sm:grid-cols-2 lg:grid-cols-3">{evidence.map((item) => <li key={item.label} className="flex items-center justify-between gap-3 bg-card px-3 py-2.5"><div><span className="text-sm text-foreground/90">{item.label}</span>{item.scope ? <span className="mt-0.5 block font-mono text-xs text-muted-foreground">{item.scope}</span> : null}</div><StatusBadge status={item.status} /></li>)}</ul><p className="mt-3 text-xs leading-relaxed text-muted-foreground">Remote CI evidence covers Verify, Fuzz Smoke, Demo, and Bench Smoke. Full campaigns are documented local evidence.</p></div></div>
    </section>
  )
}
