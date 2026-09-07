"use client"

import { useState } from "react"
import { ArrowDown, Check, Copy, Download, FileCode2, Rows3 } from "lucide-react"
import { CopybookView } from "@/components/copybook-view"
import { SectionHeading } from "@/components/section-heading"
import { referenceDemo } from "@/data/demo"
import { projectLinks } from "@/data/project"
import { cn } from "@/lib/utils"

type InputView = "copybook" | "hex"

export function DemoSection() {
  const [inputView, setInputView] = useState<InputView>("copybook")
  const [showOutput, setShowOutput] = useState(false)
  const [copied, setCopied] = useState(false)

  async function copyCommand() {
    await navigator.clipboard.writeText(referenceDemo.command)
    setCopied(true)
  }

  return (
    <section id="demo" className="scroll-mt-24 border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-20 md:px-6 md:py-28">
        <SectionHeading
          eyebrow="Try the pipeline"
          title="A small, real transformation"
          description="This reference run uses a repository-owned 105-byte fixture. The output was produced by the actual Rust CLI and packaged here so the portfolio stays useful without pretending a static page is live execution."
        />

        <div className="overflow-hidden rounded-lg border border-border bg-card shadow-[0_24px_80px_rgba(0,0,0,.22)]">
          <div className="flex flex-col gap-3 border-b border-border px-4 py-4 sm:flex-row sm:items-center sm:justify-between md:px-5">
            <div className="flex items-center gap-3">
              <span className="h-2 w-2 rounded-full bg-pass" aria-hidden />
              <div>
                <p className="text-sm font-medium text-foreground">Verified fixture-backed run</p>
                <p className="font-mono text-xs text-muted-foreground">{referenceDemo.fixture}</p>
              </div>
            </div>
            <a href={projectLinks.fixture} target="_blank" rel="noreferrer" className="text-sm text-muted-foreground underline decoration-border underline-offset-4 transition-colors hover:text-primary">Inspect fixture source</a>
          </div>

          <div className="grid lg:grid-cols-[1fr_240px_1.25fr]">
            <div className="min-w-0 border-b border-border p-4 md:p-5 lg:border-b-0 lg:border-r">
              <div className="mb-4 flex items-center justify-between gap-3">
                <div>
                  <p className="font-mono text-xs text-primary">Input</p>
                  <p className="mt-1 text-sm text-muted-foreground">{referenceDemo.report.records} fixed records · {referenceDemo.report.recordLength} bytes each</p>
                </div>
                <div role="tablist" aria-label="Fixture input view" className="flex rounded-sm border border-border bg-background p-1">
                  {(["copybook", "hex"] as const).map((view) => (
                    <button
                      key={view}
                      type="button"
                      role="tab"
                      aria-selected={inputView === view}
                      onClick={() => setInputView(view)}
                      className={cn("rounded-sm px-2.5 py-1 text-xs capitalize transition-colors", inputView === view ? "bg-elevated text-foreground" : "text-muted-foreground hover:text-foreground")}
                    >
                      {view}
                    </button>
                  ))}
                </div>
              </div>

              <div role="tabpanel" className="min-h-[330px] overflow-hidden rounded-md border border-border bg-background/70 p-3">
                {inputView === "copybook" ? (
                  <CopybookView code={referenceDemo.copybook} />
                ) : (
                  <div>
                    <p className="mb-3 font-mono text-xs text-muted-foreground">Record 1 · CP037 + DISPLAY + COMP + COMP-3</p>
                    <code className="block break-words font-mono text-sm leading-7 text-foreground/85">{referenceDemo.hexPreview}</code>
                    <p className="mt-5 border-t border-border pt-4 text-sm leading-6 text-muted-foreground">The two FILLER fields still consume bytes and affect offsets, but do not appear in the Arrow schema.</p>
                  </div>
                )}
              </div>
            </div>

            <div className="flex flex-col justify-between border-b border-border bg-background/35 p-5 lg:border-b-0 lg:border-r">
              <div>
                <p className="font-mono text-xs text-primary">Pipeline</p>
                <ol className="mt-5">
                  {referenceDemo.stages.map((stage, index) => (
                    <li key={stage} className="flex flex-col items-center text-center">
                      <div className="flex w-full items-center gap-3 rounded-sm border border-border bg-elevated/70 px-3 py-2.5 text-left">
                        <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full border border-primary/30 bg-primary/10 font-mono text-[10px] text-primary">{index + 1}</span>
                        <span className="text-sm text-foreground/90">{stage}</span>
                      </div>
                      {index < referenceDemo.stages.length - 1 ? <ArrowDown className="my-2 h-4 w-4 text-border" aria-hidden /> : null}
                    </li>
                  ))}
                </ol>
              </div>
              <button type="button" onClick={() => setShowOutput(true)} disabled={showOutput} className="mt-6 inline-flex w-full items-center justify-center gap-2 rounded-sm bg-primary px-3 py-2.5 text-sm font-semibold text-primary-foreground transition-opacity hover:opacity-90 disabled:cursor-default disabled:bg-pass disabled:text-background">
                {showOutput ? <><Check className="h-4 w-4" aria-hidden /> Output loaded</> : <><Rows3 className="h-4 w-4" aria-hidden /> Load verified output</>}
              </button>
            </div>

            <div className="min-w-0 p-4 md:p-5" aria-live="polite">
              <div className="flex items-start justify-between gap-3">
                <div>
                  <p className="font-mono text-xs text-primary">Typed result</p>
                  <p className="mt-1 text-sm text-muted-foreground">Arrow schema → Parquet file</p>
                </div>
                {showOutput ? <span className="rounded-full border border-pass/30 bg-pass/10 px-2.5 py-1 font-mono text-xs text-pass">success</span> : null}
              </div>

              {!showOutput ? (
                <div className="flex min-h-[330px] flex-col items-center justify-center px-6 text-center">
                  <FileCode2 className="h-7 w-7 text-border" aria-hidden />
                  <p className="mt-4 max-w-xs text-sm leading-6 text-muted-foreground">Load the checked-in reference output to inspect its schema, rows, and Parquet metadata.</p>
                </div>
              ) : (
                <div className="mt-5">
                  <dl className="grid grid-cols-2 gap-x-5 gap-y-4 border-b border-border pb-5 sm:grid-cols-4 lg:grid-cols-2 xl:grid-cols-4">
                    <Result label="Rows" value={referenceDemo.report.records.toString()} />
                    <Result label="Row groups" value={referenceDemo.report.rowGroups.toString()} />
                    <Result label="Format" value="Parquet" />
                    <Result label="Output" value={`${referenceDemo.report.outputBytes.toLocaleString()} B`} />
                  </dl>

                  <div className="mt-5">
                    <h3 className="text-sm font-medium text-foreground">Logical schema</h3>
                    <div className="mt-2 grid gap-x-5 gap-y-2 sm:grid-cols-2">
                      {referenceDemo.schema.map((field) => (
                        <div key={field.field} className="flex items-baseline justify-between gap-3 border-b border-border/60 py-1.5 font-mono text-xs">
                          <span className="truncate text-foreground/80" title={`SAMPLE-RECORD.HEADER-GROUP.${field.field}`}>{field.field}</span>
                          <span className="shrink-0 text-primary/85">{field.type}</span>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="mt-5">
                    <h3 className="text-sm font-medium text-foreground">Decoded rows</h3>
                    <div className="mt-2 overflow-x-auto rounded-sm border border-border">
                      <table className="min-w-[760px] border-collapse text-left font-mono text-xs">
                        <thead className="bg-elevated text-muted-foreground"><tr><th className="px-3 py-2 font-normal">CUSTOMER-NAME</th><th className="px-3 py-2 text-right font-normal">ACCOUNT</th><th className="px-3 py-2 text-right font-normal">INTEREST</th><th className="px-3 py-2 text-right font-normal">BALANCE</th><th className="px-3 py-2 text-right font-normal">RATE</th><th className="px-3 py-2 text-right font-normal">AMOUNT</th></tr></thead>
                        <tbody>{referenceDemo.rows.map((row, index) => <tr key={index} className="border-t border-border/70"><td className="px-3 py-2 text-foreground">{row.name}</td><td className="px-3 py-2 text-right text-foreground/85">{row.account}</td><td className="px-3 py-2 text-right text-foreground/85">{row.interest}</td><td className="px-3 py-2 text-right text-foreground/85">{row.balance}</td><td className="px-3 py-2 text-right text-foreground/85">{row.rate}</td><td className="px-3 py-2 text-right text-foreground/85">{row.amount}</td></tr>)}</tbody>
                      </table>
                    </div>
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="border-t border-border bg-background/30 p-4 md:p-5">
            <details>
              <summary className="cursor-pointer text-sm font-medium text-foreground marker:text-primary">Exact CLI command</summary>
              <div className="mt-4 flex flex-col gap-3 sm:flex-row sm:items-start">
                <code className="min-w-0 flex-1 overflow-x-auto rounded-sm border border-border bg-background p-3 font-mono text-xs leading-6 text-muted-foreground">{referenceDemo.command}</code>
                <button type="button" onClick={copyCommand} className="inline-flex shrink-0 items-center justify-center gap-2 rounded-sm border border-border px-3 py-2 text-sm text-foreground transition-colors hover:border-primary/40 hover:bg-primary/10">
                  {copied ? <Check className="h-4 w-4 text-pass" aria-hidden /> : <Copy className="h-4 w-4" aria-hidden />}{copied ? "Copied" : "Copy"}
                </button>
              </div>
            </details>
            <div className="mt-4 flex flex-col gap-3 border-t border-border pt-4 text-sm sm:flex-row sm:items-center sm:justify-between">
              <p className="max-w-3xl leading-6 text-muted-foreground">This interaction reveals a verified reference run; it does not execute Rust on the server. The downloadable file is the real {referenceDemo.report.outputBytes.toLocaleString()}-byte Parquet artifact generated for this page.</p>
              <a href="/sample-fixed.parquet" download className="inline-flex shrink-0 items-center gap-2 text-foreground underline decoration-border underline-offset-4 transition-colors hover:text-primary"><Download className="h-4 w-4" aria-hidden /> Download Parquet</a>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

function Result({ label, value }: { label: string; value: string }) {
  return <div><dt className="text-xs text-muted-foreground">{label}</dt><dd className="mt-1 font-mono text-sm text-foreground">{value}</dd></div>
}
