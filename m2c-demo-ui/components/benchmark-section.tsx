"use client"

import { useState } from "react"
import { Info } from "lucide-react"
import {
  benchmarkTabs,
  m3Runtime,
  m3WorkingSet,
  m4Bench,
  m5Bench,
  microBench,
  benchmarkDisclaimer,
} from "@/lib/data"
import { SectionHeading } from "@/components/section-heading"
import { BenchmarkChart } from "@/components/benchmark-chart"
import { cn } from "@/lib/utils"

type TabId = (typeof benchmarkTabs)[number]["id"]

export function BenchmarkSection() {
  const [tab, setTab] = useState<TabId>("m3")

  return (
    <section id="benchmarks" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading
          index="04 / Benchmarks"
          title="Measured performance"
          description="Local measurements against a synthetic fixed-record dataset. These figures characterize behavior of the prototype under specific conditions — they are not service-level guarantees."
        />

        <div
          role="tablist"
          aria-label="Benchmark categories"
          className="mb-6 flex flex-wrap gap-1 rounded-md border border-border bg-card p-1"
        >
          {benchmarkTabs.map((t) => (
            <button
              key={t.id}
              role="tab"
              aria-selected={tab === t.id}
              onClick={() => setTab(t.id)}
              className={cn(
                "rounded-sm px-3 py-1.5 text-sm font-medium transition-colors",
                tab === t.id
                  ? "bg-primary/15 text-primary"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {t.label}
            </button>
          ))}
        </div>

        <div className="grid gap-4 lg:grid-cols-2">
          {tab === "m3" && (
            <>
              <BenchmarkChart
                title={m3Runtime.title}
                dataset={m3Runtime.dataset}
                unit={m3Runtime.unit}
                bars={m3Runtime.bars}
              />
              <div className="rounded-md border border-border bg-card p-5">
                <div className="mb-1 flex items-baseline justify-between gap-3">
                  <h3 className="text-sm font-medium text-foreground">{m3WorkingSet.title}</h3>
                  <span className="font-mono text-[11px] text-muted-foreground">batch size</span>
                </div>
                <p className="mb-4 font-mono text-[11px] text-muted-foreground">
                  {m3WorkingSet.metric}
                </p>
                <div className="overflow-hidden rounded-sm border border-border">
                  {m3WorkingSet.rows.map((row, i) => (
                    <div
                      key={row.label}
                      className={cn(
                        "flex items-center justify-between px-3 py-2.5 font-mono text-xs",
                        i % 2 === 0 ? "bg-elevated" : "bg-card",
                      )}
                    >
                      <span className="text-muted-foreground">batch {row.label}</span>
                      <span className="tabular-nums text-foreground">{row.value}</span>
                    </div>
                  ))}
                </div>
              </div>
            </>
          )}

          {tab === "m4" && (
            <BenchmarkChart
              title="Recovery Runtime"
              dataset={m4Bench.dataset}
              unit={m4Bench.unit}
              bars={m4Bench.bars}
            />
          )}

          {tab === "m5" && (
            <BenchmarkChart
              title="Protection Overhead"
              dataset={m5Bench.dataset}
              unit={m5Bench.unit}
              bars={m5Bench.bars}
              highlightMin={false}
            />
          )}

          {tab === "micro" && (
            <BenchmarkChart
              title="Hot-path Microbenchmarks"
              dataset={microBench.dataset}
              unit={microBench.unit}
              bars={microBench.bars}
            />
          )}

          <div className="flex items-start gap-2.5 rounded-md border border-warn/25 bg-warn/[0.06] p-4 lg:col-span-2">
            <Info className="mt-0.5 h-4 w-4 shrink-0 text-warn" aria-hidden />
            <p className="text-pretty text-xs leading-relaxed text-muted-foreground">
              <span className="font-medium text-warn">Measurement scope.</span>{" "}
              {benchmarkDisclaimer}
            </p>
          </div>
        </div>
      </div>
    </section>
  )
}
