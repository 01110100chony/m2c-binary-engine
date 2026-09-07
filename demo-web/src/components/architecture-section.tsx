import { ArrowDown, ArrowRight, FileArchive, RotateCcw, Shield } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { architectureStages, projectLinks } from "@/data/project"

export function ArchitectureSection() {
  return (
    <section id="architecture" className="scroll-mt-24 border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-20 md:px-6 md:py-28">
        <SectionHeading
          eyebrow="How it works"
          title="Compile once, decode from a typed layout"
          description="The Copybook is parsed before record decoding begins. The hot path consumes resolved offsets, physical encodings, signedness, precision, scale, and logical Arrow types."
        />

        <div className="rounded-lg border border-border bg-card/60 p-4 md:p-6">
          <div className="grid gap-3 lg:grid-cols-[1fr_auto_1fr_auto_1fr_auto_1fr_auto_1fr] lg:items-stretch">
            <div className="grid gap-3">
              <FlowNode label="COBOL Copybook" note="Documented v0.1 subset" />
              <FlowNode label="Fixed-record binary" note="CP037 and supported numerics" />
            </div>
            <FlowArrow />
            {architectureStages.map((stage, index) => (
              <div key={stage.label} className="contents">
                <FlowNode label={stage.label} note={stage.note} accent={index === architectureStages.length - 1} />
                {index < architectureStages.length - 1 ? <FlowArrow /> : null}
              </div>
            ))}
          </div>
        </div>

        <div className="mt-10 grid gap-10 border-t border-border pt-10 md:grid-cols-2">
          <article className="flex gap-4">
            <RotateCcw className="mt-1 h-5 w-5 shrink-0 text-primary" aria-hidden />
            <div><h3 className="text-lg font-medium text-foreground">Resumable conversion</h3><p className="mt-2 max-w-xl text-base leading-7 text-muted-foreground">A separate local mode writes deterministic Parquet parts, a manifest, and immutable commit receipts. An explicit resume validates the committed prefix before continuing.</p></div>
          </article>
          <article className="flex gap-4">
            <Shield className="mt-1 h-5 w-5 shrink-0 text-primary" aria-hidden />
            <div><h3 className="text-lg font-medium text-foreground">Artifact protection</h3><p className="mt-2 max-w-xl text-base leading-7 text-muted-foreground">An optional Cargo feature protects a produced file separately with ML-KEM-768 key establishment, HKDF-SHA-256, and AES-256-GCM authenticated encryption.</p></div>
          </article>
        </div>
        <p className="mt-10 text-sm text-muted-foreground">The supported grammar is intentionally narrow. <a href={projectLinks.copybookSubset} target="_blank" rel="noreferrer" className="text-foreground underline decoration-border underline-offset-4 hover:text-primary">Read the exact Copybook subset.</a></p>
      </div>
    </section>
  )
}

function FlowNode({ label, note, accent = false }: { label: string; note: string; accent?: boolean }) {
  return <div className={`flex min-h-28 flex-col justify-between rounded-md border p-4 ${accent ? "border-primary/40 bg-primary/[0.06]" : "border-border bg-elevated/55"}`}><FileArchive className={`h-4 w-4 ${accent ? "text-primary" : "text-muted-foreground"}`} aria-hidden /><div className="mt-6"><p className="font-mono text-sm text-foreground">{label}</p><p className="mt-1 text-xs leading-5 text-muted-foreground">{note}</p></div></div>
}

function FlowArrow() {
  return <div className="flex items-center justify-center py-1 text-border"><ArrowDown className="h-4 w-4 lg:hidden" aria-hidden /><ArrowRight className="hidden h-4 w-4 lg:block" aria-hidden /></div>
}

