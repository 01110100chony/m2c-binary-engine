"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { Play, RotateCcw } from "lucide-react"
import { CopybookView } from "@/components/copybook-view"
import { PipelineStage } from "@/components/pipeline-stage"
import { SectionHeading } from "@/components/section-heading"
import { StatusBadge } from "@/components/status-badge"
import { TerminalPanel } from "@/components/terminal-panel"
import { simulatedDemo, type DemoStageState } from "@/data/demo"
import { cn } from "@/lib/utils"

type RunState = "idle" | "running" | "done"
const WAITING: DemoStageState[] = simulatedDemo.stages.map(() => "waiting")

export function DemoSection() {
  const [states, setStates] = useState<DemoStageState[]>(WAITING)
  const [runState, setRunState] = useState<RunState>("idle")
  const [terminalReveal, setTerminalReveal] = useState(0)
  const timers = useRef<ReturnType<typeof setTimeout>[]>([])
  const clearTimers = useCallback(() => { timers.current.forEach(clearTimeout); timers.current = [] }, [])
  useEffect(() => clearTimers, [clearTimers])

  const run = useCallback(() => {
    clearTimers()
    setRunState("running")
    setStates(WAITING)
    setTerminalReveal(2)
    const step = 620
    simulatedDemo.stages.forEach((_, index) => {
      timers.current.push(setTimeout(() => setStates((previous) => previous.map((state, itemIndex) => itemIndex === index ? "running" : state)), index * step))
      timers.current.push(setTimeout(() => {
        setStates((previous) => previous.map((state, itemIndex) => itemIndex === index ? "complete" : state))
        setTerminalReveal((count) => Math.min(count + 1, simulatedDemo.terminalLines.length))
      }, index * step + step * 0.7))
    })
    timers.current.push(setTimeout(() => { setRunState("done"); setTerminalReveal(simulatedDemo.terminalLines.length) }, simulatedDemo.stages.length * step + 200))
  }, [clearTimers])

  const reset = useCallback(() => { clearTimers(); setStates(WAITING); setRunState("idle"); setTerminalReveal(0) }, [clearTimers])
  const showResult = runState === "done"

  return (
    <section id="pipeline" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-14 md:px-6 md:py-16">
        <SectionHeading index="03 / Interactive Demo · Simulated Data" title="Local pipeline demo" description="Frontend simulation. Runtime values can later be replaced by structured M2C CLI output through --report-json in local mode." />
        <div className="overflow-hidden rounded-md border border-border bg-card">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-3">
            <div className="flex items-center gap-2"><span className="font-mono text-xs text-muted-foreground">m2c://demo/convert-parts</span><span className="rounded-sm border border-primary/30 bg-primary/10 px-2 py-0.5 font-mono text-[10px] uppercase tracking-wider text-primary">UI simulation</span></div>
            <div className="flex items-center gap-2">
              <button type="button" onClick={reset} disabled={runState === "running" || runState === "idle"} className="inline-flex items-center gap-1.5 rounded-sm border border-border px-2.5 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"><RotateCcw className="h-3.5 w-3.5" aria-hidden />Reset</button>
              <button type="button" onClick={run} disabled={runState === "running"} className="inline-flex items-center gap-1.5 rounded-sm bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-60"><Play className="h-3.5 w-3.5" aria-hidden />{runState === "running" ? "Running…" : "Run Demo"}</button>
            </div>
          </div>
          <div className="grid gap-px bg-border lg:grid-cols-3">
            <div className="bg-card p-4"><PanelLabel>Copybook input</PanelLabel><div className="mt-3 rounded-sm border border-border bg-[oklch(0.14_0.004_240)] p-3"><CopybookView code={simulatedDemo.copybook} /></div></div>
            <div className="bg-card p-4"><PanelLabel>Pipeline execution</PanelLabel><div className="mt-3 flex flex-col gap-2">{simulatedDemo.stages.map((stage, index) => <PipelineStage key={stage.key} index={index} label={stage.label} state={states[index]} />)}</div></div>
            <div className="bg-card p-4"><PanelLabel>Simulated result</PanelLabel><div className={cn("mt-3 rounded-sm border border-border bg-elevated p-3 transition-opacity", showResult ? "opacity-100" : "opacity-40")}><dl className="flex flex-col gap-2 font-mono text-xs"><ResultRow label="status">{showResult ? <StatusBadge status={simulatedDemo.report.status} /> : <Dash />}</ResultRow><ResultRow label="records">{showResult ? simulatedDemo.report.records.toLocaleString() : <Dash />}</ResultRow><ResultRow label="parts">{showResult ? simulatedDemo.report.parts : <Dash />}</ResultRow><ResultRow label="format">{showResult ? simulatedDemo.report.format : <Dash />}</ResultRow><ResultRow label="verification">{showResult ? <StatusBadge status={simulatedDemo.report.verification} /> : <Dash />}</ResultRow></dl></div><div className="mt-3"><TerminalPanel lines={simulatedDemo.terminalLines} revealCount={terminalReveal} /></div></div>
          </div>
        </div>
      </div>
    </section>
  )
}

function PanelLabel({ children }: { children: React.ReactNode }) { return <span className="font-mono text-xs uppercase tracking-widest text-primary/80">{children}</span> }
function ResultRow({ label, children }: { label: string; children: React.ReactNode }) { return <div className="flex items-center justify-between gap-3"><dt className="text-muted-foreground">{label}</dt><dd className="text-foreground">{children}</dd></div> }
function Dash() { return <span className="text-muted-foreground/50">—</span> }
