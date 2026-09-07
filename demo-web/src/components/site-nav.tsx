"use client"

import { useEffect, useState } from "react"
import { GithubMark } from "@/components/github-mark"
import { nav, projectLinks } from "@/data/project"
import { cn } from "@/lib/utils"

export function SiteNav() {
  const [active, setActive] = useState<string>(nav[0].id)

  useEffect(() => {
    const sections = nav.map((item) => document.getElementById(item.id)).filter((element): element is HTMLElement => element !== null)
    const observer = new IntersectionObserver((entries) => {
      for (const entry of entries) if (entry.isIntersecting) setActive(entry.target.id)
    }, { rootMargin: "-45% 0px -50% 0px", threshold: 0 })
    sections.forEach((element) => observer.observe(element))
    return () => observer.disconnect()
  }, [])

  return (
    <header className="sticky top-0 z-50 border-b border-border bg-background/85 backdrop-blur supports-[backdrop-filter]:bg-background/70">
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between gap-4 px-4 md:px-6">
        <a href="#overview" className="flex items-center gap-2.5">
          <span className="flex h-6 w-6 items-center justify-center rounded-sm border border-primary/40 bg-primary/10 font-mono text-xs font-bold text-primary">M2C</span>
          <span className="hidden font-mono text-sm font-medium tracking-tight text-foreground sm:inline">Binary Engine</span>
        </a>
        <nav className="hidden items-center gap-1 lg:flex" aria-label="Primary">
          {nav.map((item) => (
            <a key={item.id} href={`#${item.id}`} className={cn("rounded-sm px-2.5 py-1.5 text-sm transition-colors", active === item.id ? "text-foreground" : "text-muted-foreground hover:text-foreground")} aria-current={active === item.id ? "location" : undefined}>{item.label}</a>
          ))}
        </nav>
        <a href={projectLinks.source} target="_blank" rel="noreferrer" className="inline-flex items-center gap-2 rounded-sm border border-border bg-elevated px-3 py-1.5 text-sm font-medium text-foreground transition-colors hover:border-primary/40 hover:bg-primary/10"><GithubMark className="h-4 w-4" />GitHub</a>
      </div>
    </header>
  )
}
