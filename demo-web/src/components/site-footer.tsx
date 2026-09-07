import { GithubMark } from "@/components/github-mark"
import { projectLinks } from "@/data/project"

export function SiteFooter() {
  return (
    <footer className="bg-background">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6">
        <div className="flex flex-col items-start justify-between gap-6 md:flex-row md:items-center">
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2.5"><span className="flex h-6 w-6 items-center justify-center rounded-sm border border-primary/40 bg-primary/10 font-mono text-xs font-bold text-primary">M2C</span><span className="font-mono text-sm text-foreground">Binary Engine</span></div>
            <p className="max-w-md text-pretty text-xs leading-relaxed text-muted-foreground">Experimental portfolio prototype. Documented benchmark values are local synthetic measurements, not performance guarantees or production claims.</p>
          </div>
          <a href={projectLinks.source} target="_blank" rel="noreferrer" className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated px-3 py-2 text-sm font-medium text-foreground transition-colors hover:border-primary/40 hover:bg-primary/10"><GithubMark className="h-4 w-4" />Source on GitHub</a>
        </div>
        <nav aria-label="Technical documentation" className="mt-8 flex flex-wrap gap-x-5 gap-y-2 border-t border-border pt-6 font-mono text-xs">
          {Object.entries(projectLinks).map(([label, href]) => <a key={label} href={href} target="_blank" rel="noreferrer" className="capitalize text-muted-foreground transition-colors hover:text-primary">{label}</a>)}
        </nav>
        <div className="mt-5 font-mono text-xs text-muted-foreground">M2C Binary Engine · Rust · Arrow · Parquet · ML-KEM-768 · Prototype v0.1</div>
      </div>
    </footer>
  )
}
