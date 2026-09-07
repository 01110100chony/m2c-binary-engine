import { SectionHeading } from "@/components/section-heading"
import { learningTopics } from "@/data/project"

export function WhyBuiltSection() {
  return (
    <section className="border-b border-border">
      <div className="mx-auto grid max-w-6xl gap-10 px-4 py-20 md:px-6 md:py-24 lg:grid-cols-[.78fr_1.22fr] lg:gap-20">
        <SectionHeading eyebrow="Why I built this" title="Learning systems by following the bytes" />
        <div>
          <p className="text-pretty text-xl leading-8 text-foreground/88">I wanted one project where a physical byte layout, a typed in-memory model, a durable file format, and failure behavior all had to agree.</p>
          <p className="mt-5 max-w-3xl text-pretty text-base leading-7 text-muted-foreground">That meant working through CP037 EBCDIC, the supported DISPLAY, COMP, and COMP-3 representations, Arrow schemas, Parquet row groups, bounded batches, filesystem publication, explicit resume, benchmark design, independent validation, and optional artifact protection—without broadening the documented v0.1 subset.</p>
          <ul className="mt-8 flex flex-wrap gap-x-5 gap-y-3" aria-label="Technologies and concepts explored">
            {learningTopics.map((topic) => <li key={topic} className="border-b border-border pb-1 font-mono text-sm text-foreground/75">{topic}</li>)}
          </ul>
        </div>
      </div>
    </section>
  )
}

