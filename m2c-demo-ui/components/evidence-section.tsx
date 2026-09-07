import { evidence } from "@/lib/data"
import { StatusBadge } from "@/components/status-badge"

export function EvidenceSection() {
  return (
    <section aria-label="Verification evidence" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6">
        <div className="rounded-md border border-border bg-card p-5">
          <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
            <span className="font-mono text-[11px] uppercase tracking-widest text-primary/80">
              Verification gates
            </span>
            <span className="font-mono text-[11px] text-muted-foreground">
              {evidence.length}/{evidence.length} passing
            </span>
          </div>
          <ul className="grid grid-cols-1 gap-px overflow-hidden rounded-sm border border-border bg-border sm:grid-cols-2 lg:grid-cols-3">
            {evidence.map((item) => (
              <li
                key={item.label}
                className="flex items-center justify-between gap-3 bg-card px-3 py-2.5"
              >
                <span className="text-sm text-foreground/90">{item.label}</span>
                <StatusBadge status={item.status} />
              </li>
            ))}
          </ul>
        </div>
      </div>
    </section>
  )
}
