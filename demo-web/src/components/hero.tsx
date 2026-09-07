import { ArrowDown, ArrowUpRight } from "lucide-react"
import { GithubMark } from "@/components/github-mark"
import { publishedBenchmarks } from "@/data/benchmarks"
import { projectLinks } from "@/data/project"

export function Hero() {
  return (
    <section id="about" className="relative scroll-mt-24 overflow-hidden border-b border-border">
      <div className="pointer-events-none absolute inset-0 grid-bg opacity-[0.28]" aria-hidden />
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_74%_32%,color-mix(in_oklch,var(--primary)_12%,transparent),transparent_34%),linear-gradient(to_bottom,transparent_35%,var(--background))]" aria-hidden />
      <div className="relative mx-auto max-w-6xl px-4 py-16 md:px-6 md:py-24">
        <div className="grid items-end gap-12 lg:grid-cols-[minmax(0,1.35fr)_minmax(280px,.65fr)]">
          <div>
            <p className="mb-7 font-mono text-sm text-primary">Anthony C. · Computer Engineering</p>
            <h1 className="max-w-4xl text-balance text-4xl font-semibold leading-[1.03] tracking-[-0.04em] text-foreground sm:text-5xl md:text-6xl">
              From COBOL-described binary records to typed Arrow and Parquet data.
            </h1>
            <p className="mt-7 max-w-2xl text-pretty text-lg leading-8 text-foreground/82">
              Hi, I&apos;m Anthony. I built M2C Binary Engine in Rust to turn what I was learning about binary formats, data representation, columnar storage, failure recovery, reproducible benchmarking, and post-quantum cryptography into one end-to-end project.
            </p>
            <p className="mt-4 max-w-2xl text-pretty text-base leading-7 text-muted-foreground">
              Legacy systems still exchange fixed-record files whose layout lives in COBOL Copybooks. Modern analytical tools expect typed, self-describing data. M2C explores the bridge between those representations.
            </p>
            <div className="mt-8 flex flex-wrap items-center gap-3">
              <a href="#demo" className="inline-flex items-center gap-2 rounded-sm bg-primary px-4 py-2.5 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90">
                Try the pipeline demo <ArrowDown className="h-4 w-4" aria-hidden />
              </a>
              <a href={projectLinks.source} target="_blank" rel="noreferrer" className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated/80 px-4 py-2.5 text-sm font-medium text-foreground transition-colors hover:border-primary/50 hover:bg-primary/10">
                <GithubMark className="h-4 w-4" /> View source <ArrowUpRight className="h-3.5 w-3.5" aria-hidden />
              </a>
            </div>
            <p className="mt-6 text-sm text-muted-foreground">Educational systems/data-engineering prototype — not production software.</p>
          </div>

          <aside aria-label="Published conversion benchmark" className="border-l border-border pl-6 lg:pb-1">
            <p className="font-mono text-xs leading-5 text-muted-foreground">Local synthetic benchmark<br />{publishedBenchmarks.primary.records} records · batch {publishedBenchmarks.primary.batch.toLocaleString()}</p>
            <dl className="mt-6 grid grid-cols-2 gap-x-5 gap-y-6">
              <div className="col-span-2 border-b border-border pb-5">
                <dt className="text-sm text-muted-foreground">Median conversion</dt>
                <dd className="mt-1 font-mono text-4xl font-medium tracking-tight text-foreground">{(publishedBenchmarks.primary.medianMs / 1000).toFixed(3)} s</dd>
              </div>
              <div>
                <dt className="text-sm text-muted-foreground">Source records</dt>
                <dd className="mt-1 font-mono text-xl text-primary">{(publishedBenchmarks.primary.recordsPerSecond / 1_000_000).toFixed(2)}M/s</dd>
              </div>
              <div>
                <dt className="text-sm text-muted-foreground">Peak working set observed</dt>
                <dd className="mt-1 font-mono text-xl text-foreground">{publishedBenchmarks.primary.workingSetMiB.toFixed(2)} MiB</dd>
              </div>
            </dl>
          </aside>
        </div>
      </div>
    </section>
  )
}
