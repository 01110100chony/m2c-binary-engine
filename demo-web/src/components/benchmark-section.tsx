"use client"

import { useState } from "react"
import { ArrowUpRight, Gauge, HardDrive, Info, MemoryStick } from "lucide-react"
import { SectionHeading } from "@/components/section-heading"
import { benchmarkDisclaimer, benchmarkTabs, publishedBenchmarks } from "@/data/benchmarks"
import { projectLinks } from "@/data/project"
import { cn } from "@/lib/utils"

type TabId = (typeof benchmarkTabs)[number]["id"]

export function BenchmarkSection() {
  const [tab, setTab] = useState<TabId>("conversion")

  return (
    <section id="benchmarks" className="scroll-mt-24 border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-20 md:px-6 md:py-28">
        <div className="flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
          <SectionHeading
            eyebrow="What I measured"
            title="Performance with its boundaries intact"
            description="The published suite measures controlled local workloads, separates end-to-end conversion from in-memory decoding, and preserves every measured run."
          />
          <a href={projectLinks.benchmarks} target="_blank" rel="noreferrer" className="mb-10 inline-flex shrink-0 items-center gap-1.5 text-sm text-muted-foreground underline decoration-border underline-offset-4 transition-colors hover:text-primary">Read the benchmark report <ArrowUpRight className="h-3.5 w-3.5" aria-hidden /></a>
        </div>

        <div role="tablist" aria-label="Benchmark categories" className="mb-8 flex w-fit max-w-full overflow-x-auto rounded-sm border border-border bg-card p-1">
          {benchmarkTabs.map((item) => (
            <button key={item.id} id={`tab-${item.id}`} role="tab" aria-selected={tab === item.id} aria-controls={`panel-${item.id}`} onClick={() => setTab(item.id)} className={cn("shrink-0 rounded-sm px-3 py-2 text-sm transition-colors", tab === item.id ? "bg-elevated text-foreground" : "text-muted-foreground hover:text-foreground")}>{item.label}</button>
          ))}
        </div>

        <div id={`panel-${tab}`} role="tabpanel" aria-labelledby={`tab-${tab}`}>
          {tab === "conversion" ? <ConversionPanel /> : null}
          {tab === "recovery" ? <RecoveryPanel /> : null}
          {tab === "details" ? <DetailsPanel /> : null}
        </div>

        <div className="mt-8 flex items-start gap-3 border-t border-border pt-5">
          <Info className="mt-0.5 h-4 w-4 shrink-0 text-warn" aria-hidden />
          <p className="max-w-4xl text-sm leading-6 text-muted-foreground">{benchmarkDisclaimer}</p>
        </div>
      </div>
    </section>
  )
}

function ConversionPanel() {
  const maxRuntime = Math.max(...publishedBenchmarks.conversion.map((item) => item.medianMs))
  const maxWorkingSet = Math.max(...publishedBenchmarks.conversion.map((item) => item.workingSetMiB))

  return (
    <div>
      <div className="mb-6 flex flex-col gap-2 border-b border-border pb-5 sm:flex-row sm:items-end sm:justify-between">
        <div><h3 className="text-lg font-medium text-foreground">Batch-size tradeoff</h3><p className="mt-1 text-sm text-muted-foreground">{publishedBenchmarks.context}</p></div>
        <p className="font-mono text-xs text-muted-foreground">Median wall-clock time</p>
      </div>
      <div className="divide-y divide-border border-y border-border">
        {publishedBenchmarks.conversion.map((item) => (
          <article key={item.batch} className="grid gap-5 py-6 md:grid-cols-[120px_1fr_1fr_150px] md:items-center">
            <div><p className="text-xs text-muted-foreground">Batch</p><h4 className="mt-1 font-mono text-xl text-foreground">{item.batch.toLocaleString()}</h4></div>
            <MeasureBar label="Runtime" value={`${item.medianMs.toLocaleString("en-US", { minimumFractionDigits: 2 })} ms`} width={(item.medianMs / maxRuntime) * 100} tone="primary" />
            <MeasureBar label="Observed peak working set" value={`${item.workingSetMiB.toFixed(2)} MiB`} width={(item.workingSetMiB / maxWorkingSet) * 100} tone="muted" />
            <div className="md:text-right"><p className="text-xs text-muted-foreground">Source records</p><p className="mt-1 font-mono text-sm text-foreground">{item.recordsPerSecond.toLocaleString()}/s</p></div>
          </article>
        ))}
      </div>
      <div className="mt-7 grid gap-4 border-l-2 border-primary/60 pl-5 md:grid-cols-[180px_1fr]">
        <p className="font-mono text-sm text-primary">What I learned</p>
        <p className="max-w-3xl text-base leading-7 text-foreground/80">Very small batches substantially increased both runtime and observed working set in this test. The behavior is consistent with the large number of Parquet row groups and associated metadata retained by the writer in this workload.</p>
      </div>
    </div>
  )
}

function RecoveryPanel() {
  const single = publishedBenchmarks.primary
  const recoverable = publishedBenchmarks.recovery[1]
  const max = Math.max(single.medianMs, recoverable.medianMs)

  return (
    <div className="grid gap-8 lg:grid-cols-[1.25fr_.75fr]">
      <div>
        <h3 className="text-lg font-medium text-foreground">Resume instead of restart</h3>
        <p className="mt-2 max-w-2xl text-base leading-7 text-muted-foreground">At batch 65,536, the recoverable path publishes 46 deterministic Parquet parts with commit receipts. That recoverability has a measurable cost.</p>
        <div className="mt-7 space-y-6">
          <MeasureBar label="Single-file conversion" value={`${single.medianMs.toLocaleString("en-US", { minimumFractionDigits: 2 })} ms`} width={(single.medianMs / max) * 100} tone="primary" />
          <MeasureBar label="Recoverable multipart conversion" value={`${recoverable.medianMs.toLocaleString("en-US", { minimumFractionDigits: 2 })} ms`} width={(recoverable.medianMs / max) * 100} tone="muted" />
        </div>
        <dl className="mt-8 grid grid-cols-2 gap-5 border-t border-border pt-6 sm:grid-cols-4">
          <Metric label="Output parts" value={recoverable.parts.toString()} />
          <Metric label="Records / second" value={recoverable.recordsPerSecond.toLocaleString()} />
          <Metric label="Source throughput" value={`${recoverable.sourceMiBPerSecond.toFixed(2)} MiB/s`} />
          <Metric label="Peak WS observed" value={`${recoverable.workingSetMiB.toFixed(2)} MiB`} />
        </dl>
      </div>
      <aside className="border-t border-border pt-6 lg:border-l lg:border-t-0 lg:pl-8 lg:pt-0">
        <p className="font-mono text-xs text-primary">Hundreds of parts</p>
        <p className="mt-4 font-mono text-3xl text-foreground">733</p>
        <p className="mt-1 text-sm text-muted-foreground">parts at batch 4,096</p>
        <p className="mt-5 text-sm leading-6 text-foreground/75">That scenario measured 291,058.86 ms median. The result is consistent with publication and filesystem overhead becoming important when hundreds of files and commit receipts are created; it is not presented as a profiled single-cause conclusion.</p>
      </aside>
    </div>
  )
}

function DetailsPanel() {
  const decoder = publishedBenchmarks.mixedDecoder

  return (
    <div className="grid gap-px overflow-hidden rounded-md border border-border bg-border lg:grid-cols-2">
      <article className="bg-card p-6 md:p-8">
        <div className="flex items-center gap-2 text-primary"><Gauge className="h-4 w-4" aria-hidden /><p className="font-mono text-xs">In-memory mixed-record decoder</p></div>
        <p className="mt-5 font-mono text-4xl tracking-tight text-foreground">{(decoder.recordsPerSecond / 1_000_000).toFixed(2)}M <span className="text-base text-muted-foreground">records/s</span></p>
        <dl className="mt-6 grid grid-cols-2 gap-5">
          <Metric label="Per iteration" value={`${decoder.recordsPerIteration} records`} />
          <Metric label="Median" value={`${decoder.medianNs.toLocaleString()} ns`} />
          <Metric label="Effective input" value={`${decoder.sourceMiBPerSecond.toFixed(2)} MiB/s`} />
          <Metric label="Boundary" value="Memory only" />
        </dl>
        <p className="mt-6 text-sm leading-6 text-muted-foreground">Excludes file I/O and Parquet writing. This is a decoder microbenchmark, not end-to-end pipeline throughput.</p>
      </article>
      <article className="bg-card p-6 md:p-8">
        <div className="flex items-center gap-2 text-primary"><HardDrive className="h-4 w-4" aria-hidden /><p className="font-mono text-xs">64 MiB artifact protection</p></div>
        <div className="mt-5 space-y-5">
          {publishedBenchmarks.protection.map((item) => <div key={item.operation} className="grid grid-cols-[1fr_auto] gap-4 border-b border-border pb-4"><div><p className="text-sm text-foreground">{item.operation}</p><p className="mt-1 text-xs text-muted-foreground">{item.sourceMiBPerSecond.toFixed(2)} MiB/s source throughput</p></div><p className="font-mono text-lg text-foreground">{item.medianMs.toLocaleString("en-US", { minimumFractionDigits: 2 })} ms</p></div>)}
        </div>
        <div className="mt-5 flex gap-3"><MemoryStick className="mt-0.5 h-4 w-4 shrink-0 text-muted-foreground" aria-hidden /><p className="text-sm leading-6 text-muted-foreground">Observed working set was 5.27 MiB in both measured 64 MiB scenarios. This is an empirical observation from this benchmark, not a universal memory bound. Protect timings also showed substantial run-to-run dispersion.</p></div>
      </article>
    </div>
  )
}

function MeasureBar({ label, value, width, tone }: { label: string; value: string; width: number; tone: "primary" | "muted" }) {
  return <div><div className="mb-2 flex items-baseline justify-between gap-4"><span className="text-xs text-muted-foreground">{label}</span><span className="font-mono text-xs text-foreground">{value}</span></div><div className="h-2 overflow-hidden rounded-full bg-elevated"><div className={cn("h-full min-w-1 rounded-full", tone === "primary" ? "bg-primary" : "bg-muted-foreground/55")} style={{ width: `${Math.max(width, 2)}%` }} /></div></div>
}

function Metric({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-xs text-muted-foreground">{label}</dt><dd className="mt-1 font-mono text-sm text-foreground">{value}</dd></div>
}
