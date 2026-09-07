"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { Play, RotateCcw } from "lucide-react"
import {
  copybookExample,
  demoStages,
  demoReport,
  terminalLines,
  type DemoStageState,
} from "@/lib/data"
import { SectionHeading } from "@/components/section-heading"
import { CopybookView } from "@/components/copybook-view"
import { PipelineStage } from "@/components/pipeline-stage"
import { TerminalPanel } from "@/components/terminal-panel"
import { StatusBadge } from "@/components/status-badge"
import { cn } from "@/lib/utils"

type RunState = "idle" | "running" | "done"

const WAITING: DemoStageState[] = demoStages.map(() => "waiting")

export function DemoSection() {
  const [states, setStates] = useState<DemoStageState[]>(WAITING)
  const [runState, setRunState] = useState<RunState>("idle")
  const [terminalReveal, setTerminalReveal] = useState(0)
  const timers = useRef<ReturnType<typeof setTimeout>[]>([])

  const clearTimers = useCallback(() => {
    timers.current.forEach(clearTimeout)
    timers.current = []
  }, [])

  useEffect(() => clearTimers, [clearTimers])

  const run = useCallback(() => {
    clearTimers()
    setRunState("running")
    setStates(WAITING)
    setTerminalReveal(2) // reveal the command immediately

    const step = 620
    demoStages.forEach((_, i) => {
      timers.current.push(
        setTimeout(() => {
          setStates((prev) => prev.map((s, idx) => (idx === i ? "running" : s)))
        }, i * step),
      )
      timers.current.push(
        setTimeout(() => {
          setStates((prev) => prev.map((s, idx) => (idx === i ? "complete" : s)))
          setTerminalReveal((r) => Math.min(r + 1, terminalLines.length))
        }, i * step + step * 0.7),
      )
    })

    timers.current.push(
      setTimeout(() => {
        setRunState("done")
        setTerminalReveal(terminalLines.length)
      }, demoStages.length * step + 200),
    )
  }, [clearTimers])

  const reset = useCallback(() => {
    clearTimers()
    setStates(WAITING)
    setRunState("idle")
    setTerminalReveal(0)
  }, [clearTimers])

  const showResult = runState === "done"

  return (
    <section id="pipeline" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading
          index="03 / Live Demo"
          title="Local pipeline demo"
          description="A front-end mock of an in-process conversion run. Demo values are isolated so they can later be replaced with real JSON emitted by the M2C CLI via --report-json."
        />

        <div className="overflow-hidden rounded-md border border-border bg-card">
          <div className="flex items-center justify-between gap-4 border-b border-border px-4 py-3">
            <span className="font-mono text-xs text-muted-foreground">m2c://demo/convert-parts</span>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={reset}
                disabled={runState === "running" || runState === "idle"}
                className="inline-flex items-center gap-1.5 rounded-sm border border-border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
              >
                <RotateCcw className="h-3.5 w-3.5" aria-hidden />
                Reset
              </button>
              <button
                type="button"
                onClick={run}
                disabled={runState === "running"}
                className="inline-flex items-center gap-1.5 rounded-sm bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"
              >
                <Play className="h-3.5 w-3.5" aria-hidden />
                {runState === "running" ? "Running…" : "Run Demo"}
              </button>
            </div>
          </div>

          <div className="grid gap-px bg-border lg:grid-cols-3">
            {/* LEFT — Copybook input */}
            <div className="bg-card p-4">
              <PanelLabel>Copybook input</PanelLabel>
              <div className="mt-3 rounded-sm border border-border bg-[oklch(0.14_0.004_240)] p-3">
                <CopybookView code={copybookExample} />
              </div>
            </div>

            {/* CENTER — Pipeline execution */}
            <div className="bg-card p-4">
              <PanelLabel>Pipeline execution</PanelLabel>
              <div className="mt-3 flex flex-col gap-2">
                {demoStages.map((stage, i) => (
                  <PipelineStage key={stage.key} index={i} label={stage.label} state={states[i]} />
                ))}
              </div>
            </div>

            {/* RIGHT — Result summary */}
            <div className="bg-card p-4">
              <PanelLabel>Result summary</PanelLabel>
              <div
                className={cn(
                  "mt-3 rounded-sm border border-border bg-elevated p-3 transition-opacity",
                  showResult ? "opacity-100" : "opacity-40",
                )}
              >
                <dl className="flex flex-col gap-2 font-mono text-xs">
                  <ResultRow label="status">
                    {showResult ? <StatusBadge status={demoReport.status} /> : <Dash />}
                  </ResultRow>
                  <ResultRow label="records">
                    {showResult ? demoReport.records.toLocaleString() : <Dash />}
                  </ResultRow>
                  <ResultRow label="parts">{showResult ? demoReport.parts : <Dash />}</ResultRow>
                  <ResultRow label="format">{showResult ? demoReport.format : <Dash />}</ResultRow>
                  <ResultRow label="verification">
                    {showResult ? <StatusBadge status={demoReport.verification} /> : <Dash />}
                  </ResultRow>
                </dl>
              </div>

              <div className="mt-3">
                <TerminalPanel lines={terminalLines} revealCount={terminalReveal} />
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

function PanelLabel({ children }: { children: React.ReactNode }) {
  return (
    <span className="font-mono text-[11px] uppercase tracking-widest text-primary/80">
      {children}
    </span>
  )
}

function ResultRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-foreground">{children}</dd>
    </div>
  )
}

function Dash() {
  return <span className="text-muted-foreground/50">—</span>
}
