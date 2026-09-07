import { GITHUB_URL } from "@/lib/data"
import { GithubMark } from "@/components/github-mark"

export function SiteFooter() {
  return (
    <footer className="bg-background">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6">
        <div className="flex flex-col items-start justify-between gap-6 md:flex-row md:items-center">
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2.5">
              <span className="flex h-6 w-6 items-center justify-center rounded-sm border border-primary/40 bg-primary/10 font-mono text-[11px] font-bold text-primary">
                M2C
              </span>
              <span className="font-mono text-sm text-foreground">Binary Engine</span>
            </div>
            <p className="max-w-md text-pretty text-xs leading-relaxed text-muted-foreground">
              Experimental research prototype. Benchmarks reflect local measurements against
              synthetic data and do not constitute performance guarantees.
            </p>
          </div>

          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated px-3 py-2 text-sm font-medium text-foreground transition-colors hover:border-primary/40 hover:bg-primary/10"
          >
            <GithubMark className="h-4 w-4" />
            Source on GitHub
          </a>
        </div>

        <div className="mt-8 border-t border-border pt-6 font-mono text-[11px] text-muted-foreground">
          M2C Binary Engine · Rust · Arrow · Parquet · ML-KEM-768 · v0.1 experimental
        </div>
      </div>
    </footer>
  )
}
