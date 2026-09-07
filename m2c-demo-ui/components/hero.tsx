import { Play } from "lucide-react"
import { GITHUB_URL } from "@/lib/data"
import { PipelineFlow } from "@/components/pipeline-flow"
import { GithubMark } from "@/components/github-mark"

export function Hero() {
  return (
    <section id="overview" className="relative overflow-hidden border-b border-border">
      <div className="pointer-events-none absolute inset-0 grid-bg opacity-[0.35]" aria-hidden />
      <div
        className="pointer-events-none absolute inset-0 bg-gradient-to-b from-transparent via-background/40 to-background"
        aria-hidden
      />
      <div className="relative mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-20">
        <div className="grid items-center gap-10 lg:grid-cols-[1.1fr_1fr]">
          <div className="flex flex-col gap-5">
            <span className="inline-flex w-fit items-center gap-2 rounded-sm border border-border bg-card px-2.5 py-1 font-mono text-[11px] text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-pass" aria-hidden />
              Experimental Rust prototype · v0.1
            </span>

            <h1 className="text-balance text-3xl font-semibold tracking-tight text-foreground md:text-5xl">
              M2C Binary Engine
            </h1>

            <p className="text-pretty text-base text-foreground/90 md:text-lg">
              Legacy binary data{" "}
              <span className="text-primary">→ typed, recoverable analytical artifacts.</span>
            </p>

            <p className="max-w-xl text-pretty text-sm leading-relaxed text-muted-foreground">
              An experimental Rust pipeline for decoding fixed-record mainframe data using COBOL
              Copybooks, producing Arrow/Parquet outputs with deterministic recovery and optional
              post-quantum artifact protection.
            </p>

            <div className="mt-1 flex flex-wrap items-center gap-3">
              <a
                href="#demo"
                className="inline-flex items-center gap-2 rounded-sm bg-primary px-4 py-2 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90"
              >
                <Play className="h-4 w-4" aria-hidden />
                Run Demo
              </a>
              <a
                href={GITHUB_URL}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated px-4 py-2 text-sm font-medium text-foreground transition-colors hover:border-primary/40"
              >
                <GithubMark className="h-4 w-4" />
                View GitHub
              </a>
            </div>
          </div>

          <PipelineFlow />
        </div>
      </div>
    </section>
  )
}
