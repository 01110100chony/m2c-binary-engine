import { MetricCard } from "@/components/metric-card"
import { measuredEvidence } from "@/data/benchmarks"

export function MetricsSection() {
  return (
    <section aria-label="Measured system evidence" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {measuredEvidence.heroMetrics.map((metric) => <MetricCard key={metric.label} {...metric} />)}
        </div>
      </div>
    </section>
  )
}
