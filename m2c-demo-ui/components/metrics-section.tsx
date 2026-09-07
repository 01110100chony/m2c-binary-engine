import { metrics } from "@/lib/data"
import { MetricCard } from "@/components/metric-card"

export function MetricsSection() {
  return (
    <section aria-label="System status and key metrics" className="border-b border-border">
      <div className="mx-auto max-w-6xl px-4 py-10 md:px-6">
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {metrics.map((m) => (
            <MetricCard key={m.label} {...m} />
          ))}
        </div>
      </div>
    </section>
  )
}
