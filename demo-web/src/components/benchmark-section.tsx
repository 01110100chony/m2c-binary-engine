"use client"

import { useState } from "react"
import { Info } from "lucide-react"
import { BenchmarkChart } from "@/components/benchmark-chart"
import { SectionHeading } from "@/components/section-heading"
import { benchmarkDisclaimer, benchmarkTabs, measuredEvidence } from "@/data/benchmarks"
import { cn } from "@/lib/utils"

type TabId = (typeof benchmarkTabs)[number]["id"]

export function BenchmarkSection() {
  const [tab, setTab] = useState<TabId>("m3")
  return (
    <section id="benchmarks" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading index="04 / Benchmarks" title="Measured performance" description="Documented local M6 evidence from synthetic fixed-record scenarios. Values are presented in their measured units without invented per-record conversions." />
        <div role="tablist" aria-label="Benchmark categories" className="mb-6 flex flex-wrap gap-1 rounded-md border border-border bg-card p-1">
          {benchmarkTabs.map((item) => <button key={item.id} id={`tab-${item.id}`} role="tab" aria-selected={tab === item.id} aria-controls={`panel-${item.id}`} onClick={() => setTab(item.id)} className={cn("rounded-sm px-3 py-1.5 text-sm font-medium transition-colors", tab === item.id ? "bg-primary/15 text-primary" : "text-muted-foreground hover:text-foreground")}>{item.label}</button>)}
        </div>

        <div id={`panel-${tab}`} role="tabpanel" aria-labelledby={`tab-${tab}`} className="grid gap-4 lg:grid-cols-2">
          {tab === "m3" && <>
            <BenchmarkChart {...measuredEvidence.m3Runtime} />
            <div className="rounded-md border border-border bg-card p-5">
              <div className="mb-1 flex items-baseline justify-between gap-3"><h3 className="text-sm font-medium text-foreground">Observed working set</h3><span className="font-mono text-xs text-muted-foreground">batch size</span></div>
              <p className="mb-4 font-mono text-xs text-muted-foreground">Greatest PeakWorkingSet64 observed during local execution</p>
              <div className="overflow-hidden rounded-sm border border-border">{measuredEvidence.m3WorkingSet.map((row, index) => <div key={row.label} className={cn("flex items-center justify-between px-3 py-2.5 font-mono text-xs", index % 2 === 0 ? "bg-elevated" : "bg-card")}><span className="text-muted-foreground">batch {row.label}</span><span className="tabular-nums text-foreground">{row.value}</span></div>)}</div>
            </div>
          </>}

          {tab === "m4" && <>
            <BenchmarkChart {...measuredEvidence.m4Create} />
            <BenchmarkChart {...measuredEvidence.m4Resume} />
            <div className="overflow-x-auto rounded-md border border-border bg-card p-5 lg:col-span-2">
              <h3 className="mb-1 text-sm font-medium text-foreground">3,000,000-record scenarios</h3>
              <p className="mb-4 font-mono text-xs text-muted-foreground">Measured with batch 65,536</p>
              <table className="w-full min-w-[620px] border-collapse text-left text-sm"><thead className="font-mono text-xs text-muted-foreground"><tr><th className="border-b border-border px-3 py-2 font-normal">Operation</th><th className="border-b border-border px-3 py-2 font-normal">Records</th><th className="border-b border-border px-3 py-2 font-normal">Batch</th><th className="border-b border-border px-3 py-2 text-right font-normal">Median</th><th className="border-b border-border px-3 py-2 text-right font-normal">Observed working set</th></tr></thead><tbody>{measuredEvidence.m4Scale.map((row) => <tr key={row.operation}><td className="border-b border-border/60 px-3 py-2.5 text-foreground">{row.operation}</td><td className="border-b border-border/60 px-3 py-2.5 font-mono text-xs text-muted-foreground">{row.records}</td><td className="border-b border-border/60 px-3 py-2.5 font-mono text-xs text-muted-foreground">{row.batch}</td><td className="border-b border-border/60 px-3 py-2.5 text-right font-mono text-xs text-foreground">{row.median}</td><td className="border-b border-border/60 px-3 py-2.5 text-right font-mono text-xs text-foreground">{row.workingSet}</td></tr>)}</tbody></table>
            </div>
          </>}

          {tab === "m5" && <><BenchmarkChart {...measuredEvidence.m5} highlightMin={false} /><div className="rounded-md border border-border bg-card p-5"><h3 className="text-sm font-medium text-foreground">Measurement boundary</h3><p className="mt-3 text-sm leading-relaxed text-muted-foreground">Protect and unprotect are separate M5 file operations over a 64 MiB payload. No cryptographic primitive microbenchmarks or M3/M4 integration costs are inferred.</p></div></>}

          {tab === "micro" && <div className="overflow-x-auto rounded-md border border-border bg-card p-5 lg:col-span-2">
            <div className="mb-4"><h3 className="text-sm font-medium text-foreground">Compile and decode scenarios</h3><p className="mt-1 font-mono text-xs text-muted-foreground">ns / iteration · compile includes parse + compile · decode uses a precompiled layout</p></div>
            <table className="w-full min-w-[560px] border-collapse text-left"><thead className="font-mono text-xs text-muted-foreground"><tr><th className="border-b border-border px-3 py-2 font-normal">Fixture</th><th className="border-b border-border px-3 py-2 text-right font-normal">Compile</th><th className="border-b border-border px-3 py-2 text-right font-normal">Decode</th><th className="border-b border-border px-3 py-2 text-right font-normal">Records / decode</th></tr></thead><tbody>{measuredEvidence.micro.map((row) => <tr key={row.fixture}><td className="border-b border-border/60 px-3 py-3 text-sm text-foreground">{row.fixture}</td><td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums text-foreground">{row.compile}</td><td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums text-foreground">{row.decode}</td><td className="border-b border-border/60 px-3 py-3 text-right font-mono text-xs tabular-nums text-muted-foreground">{row.records}</td></tr>)}</tbody></table>
          </div>}

          <div className="flex items-start gap-2.5 rounded-md border border-warn/25 bg-warn/[0.06] p-4 lg:col-span-2"><Info className="mt-0.5 h-4 w-4 shrink-0 text-warn" aria-hidden /><p className="text-pretty text-xs leading-relaxed text-muted-foreground"><span className="font-medium text-warn">Measurement scope.</span> {benchmarkDisclaimer}</p></div>
        </div>
      </div>
    </section>
  )
}
