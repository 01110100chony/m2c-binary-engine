import { GithubMark } from "@/components/github-mark"
import { projectLinks } from "@/data/project"

export function SiteFooter() {
  return (
    <footer className="bg-background">
      <div className="mx-auto max-w-6xl px-4 py-12 md:px-6 md:py-16">
        <div className="flex flex-col items-start justify-between gap-6 md:flex-row md:items-center">
          <div><p className="max-w-xl text-pretty text-lg leading-7 text-foreground/85">Built by Anthony as a Computer Engineering portfolio project focused on systems and data infrastructure.</p><p className="mt-2 text-sm text-muted-foreground">Educational prototype · not production software</p></div>
          <a href={projectLinks.source} target="_blank" rel="noreferrer" className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated px-3 py-2 text-sm font-medium text-foreground transition-colors hover:border-primary/40 hover:bg-primary/10"><GithubMark className="h-4 w-4" />View source</a>
        </div>
        <nav aria-label="Technical documentation" className="mt-10 flex flex-wrap gap-x-6 gap-y-3 border-t border-border pt-6 text-sm">
          {[{ label: "GitHub", href: projectLinks.source }, { label: "Architecture", href: projectLinks.architecture }, { label: "Benchmarks", href: projectLinks.benchmarks }, { label: "Compatibility validation", href: projectLinks.validation }].map((item) => <a key={item.label} href={item.href} target="_blank" rel="noreferrer" className="text-muted-foreground transition-colors hover:text-primary">{item.label}</a>)}
        </nav>
        <div className="mt-6 font-mono text-xs text-muted-foreground">Rust · Arrow · Parquet · Fixed-record data · Prototype v0.1</div>
      </div>
    </footer>
  )
}
