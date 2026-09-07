import { SectionHeading } from "@/components/section-heading"
import { StatusBadge } from "@/components/status-badge"
import { milestones } from "@/data/project"

export function MilestonesSection() {
  return (
    <section id="milestones" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16"><SectionHeading index="07 / Roadmap" title="Milestone status" description="M0–M6 are complete for the experimental portfolio prototype. This status does not imply production readiness." /><ol className="relative flex flex-col">{milestones.map((milestone, index) => <li key={milestone.id} className="flex gap-4"><div className="flex flex-col items-center"><span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-sm border border-pass/30 bg-pass/10 font-mono text-xs font-semibold text-pass">{milestone.id}</span>{index < milestones.length - 1 ? <span className="w-px flex-1 bg-border" aria-hidden /> : null}</div><div className="flex flex-1 items-center justify-between gap-3 border-b border-border pb-5 pt-1.5"><span className="text-sm font-medium text-foreground">{milestone.title}</span><StatusBadge status={milestone.status} /></div></li>)}</ol><div className="mt-6 flex items-center justify-between rounded-md border border-pass/30 bg-pass/[0.06] px-4 py-3"><div><span className="font-mono text-xs uppercase tracking-wider text-muted-foreground">Final state</span><p className="mt-1 text-sm font-medium text-foreground">Prototype v0.1</p></div><StatusBadge status="PASS" label="COMPLETE" /></div></div>
    </section>
  )
}
