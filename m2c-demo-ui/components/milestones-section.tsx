import { milestones } from "@/lib/data"
import { SectionHeading } from "@/components/section-heading"
import { StatusBadge } from "@/components/status-badge"

export function MilestonesSection() {
  return (
    <section id="milestones" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading
          index="07 / Roadmap"
          title="Milestone status"
          description="Development is organized into verifiable milestones, each gated by the evidence suite before it is considered complete."
        />

        <ol className="relative flex flex-col">
          {milestones.map((m, i) => (
            <li key={m.id} className="flex gap-4">
              <div className="flex flex-col items-center">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-sm border border-pass/30 bg-pass/10 font-mono text-[11px] font-semibold text-pass">
                  {m.id}
                </span>
                {i < milestones.length - 1 ? (
                  <span className="w-px flex-1 bg-border" aria-hidden />
                ) : null}
              </div>
              <div className="flex flex-1 items-center justify-between gap-3 border-b border-border pb-5 pt-1.5">
                <span className="text-sm font-medium text-foreground">{m.title}</span>
                <StatusBadge status={m.status} />
              </div>
            </li>
          ))}
        </ol>
      </div>
    </section>
  )
}
