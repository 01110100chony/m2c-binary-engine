import { FileCheck2, LockKeyhole } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"

export function EngineeringDecisions() {
  return (
    <section className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-20 md:px-6 md:py-28">
        <SectionHeading eyebrow="Engineering decisions" title="Make failure and protection explicit" description="Two capabilities sit beside the core conversion path. Each has a narrow contract and avoids implying infrastructure the project does not provide." />
        <div className="grid gap-px overflow-hidden rounded-lg border border-border bg-border md:grid-cols-2">
          <article className="bg-card p-6 md:p-8">
            <FileCheck2 className="h-5 w-5 text-primary" aria-hidden />
            <h3 className="mt-6 text-2xl font-medium tracking-tight text-foreground">Resume instead of restart</h3>
            <p className="mt-4 text-base leading-7 text-muted-foreground">Large local conversions should not necessarily restart from zero after a process interruption. The recoverable path makes publication state visible and validates it before continuation.</p>
            <ul className="mt-7 grid gap-2 border-t border-border pt-6 font-mono text-sm text-foreground/75 sm:grid-cols-2">
              <li>Deterministic part names</li><li>Manifest</li><li>Immutable commit receipts</li><li>Validation before continuation</li><li>Explicit resume</li>
            </ul>
            <p className="mt-6 text-sm leading-6 text-muted-foreground">This is local process-failure recovery, not distributed fault tolerance.</p>
          </article>
          <article className="bg-card p-6 md:p-8">
            <LockKeyhole className="h-5 w-5 text-primary" aria-hidden />
            <h3 className="mt-6 text-2xl font-medium tracking-tight text-foreground">Protect artifacts separately</h3>
            <p className="mt-4 text-base leading-7 text-muted-foreground">Protection is intentionally separate from conversion. A produced artifact can be read as input to a closed, versioned envelope without coupling cryptographic formats to Arrow or Parquet semantics.</p>
            <ul className="mt-7 grid gap-2 border-t border-border pt-6 font-mono text-sm text-foreground/75 sm:grid-cols-2">
              <li>ML-KEM-768</li><li>HKDF-SHA-256</li><li>AES-256-GCM</li><li>Optional <code>pqc</code> feature</li>
            </ul>
            <p className="mt-6 text-sm leading-6 text-muted-foreground">This is an experimental portfolio implementation. It is not a security audit or security certification.</p>
          </article>
        </div>
      </div>
    </section>
  )
}
