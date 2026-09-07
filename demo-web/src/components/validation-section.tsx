import { ArrowUpRight, Check, Equal } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { projectLinks } from "@/data/project"

export function ValidationSection() {
  return (
    <section id="validation" className="scroll-mt-24 border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-20 md:px-6 md:py-28">
        <SectionHeading
          eyebrow="Checked against another implementation"
          title="100 out of 100 records matched"
          description="The same externally generated mainframe-style binary dataset was decoded independently by M2C and by Apache Spark with Cobrix. This is a correctness comparison, not a performance comparison."
        />

        <div className="grid gap-px overflow-hidden rounded-lg border border-border bg-border lg:grid-cols-[.7fr_1.3fr]">
          <div className="flex flex-col justify-between bg-primary/[0.055] p-6 md:p-8">
            <div>
              <p className="font-mono text-6xl font-medium tracking-[-0.05em] text-foreground">100<span className="text-primary">/100</span></p>
              <p className="mt-2 text-base text-foreground/80">semantically identical records</p>
            </div>
            <dl className="mt-10 grid grid-cols-2 gap-5 border-t border-primary/20 pt-6">
              <ValidationMetric label="Fields compared" value="4 per record" />
              <ValidationMetric label="Text encoding" value="CP037 EBCDIC" />
              <ValidationMetric label="Spark" value="4.0.1" />
              <ValidationMetric label="Cobrix" value="2.9.4" />
            </dl>
          </div>

          <div className="bg-card p-6 md:p-8">
            <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
              <EngineLabel name="M2C" detail="Rust · Arrow 53" />
              <Equal className="h-5 w-5 shrink-0 text-primary" aria-hidden />
              <EngineLabel name="Spark / Cobrix" detail="Independent decode" />
            </div>
            <ul className="mt-8 space-y-3">
              {["CP037 text matched exactly", "COMP-3 packed decimals matched exactly", "Column names and numeric representations normalized as documented", "All four fields compared across every record"].map((item) => <li key={item} className="flex gap-3 text-sm leading-6 text-foreground/80"><Check className="mt-1 h-4 w-4 shrink-0 text-pass" aria-hidden />{item}</li>)}
            </ul>

            <div className="mt-8 border-l-2 border-warn/50 pl-4">
              <p className="text-sm leading-6 text-muted-foreground">The external Copybook writes <code className="font-mono text-foreground">PIC S9(7)V99 COMP-3</code>. M2C decoded the same binary data using the semantically equivalent <code className="font-mono text-foreground">PIC S9(7)V9(2) COMP-3</code> form accepted by its documented subset.</p>
            </div>
            <p className="mt-6 text-sm leading-6 text-muted-foreground">This does not establish full COBOL support, OCCURS or REDEFINES support, variable-record support, or production-mainframe compatibility in general.</p>
            <a href={projectLinks.validation} target="_blank" rel="noreferrer" className="mt-7 inline-flex items-center gap-1.5 text-sm text-foreground underline decoration-border underline-offset-4 transition-colors hover:text-primary">Read the compatibility report <ArrowUpRight className="h-3.5 w-3.5" aria-hidden /></a>
          </div>
        </div>
      </div>
    </section>
  )
}

function ValidationMetric({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-xs text-muted-foreground">{label}</dt><dd className="mt-1 font-mono text-sm text-foreground">{value}</dd></div>
}

function EngineLabel({ name, detail }: { name: string; detail: string }) {
  return <div className="flex-1 rounded-sm border border-border bg-elevated/55 px-4 py-3"><p className="font-mono text-sm text-foreground">{name}</p><p className="mt-1 text-xs text-muted-foreground">{detail}</p></div>
}

