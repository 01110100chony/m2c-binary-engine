import { BenchmarkSection } from "@/components/benchmark-section"
import { DemoSection } from "@/components/demo-section"
import { EvidenceSection } from "@/components/evidence-section"
import { Hero } from "@/components/hero"
import { MetricsSection } from "@/components/metrics-section"
import { MilestonesSection } from "@/components/milestones-section"
import { RecoverySection } from "@/components/recovery-section"
import { SecuritySection } from "@/components/security-section"
import { SiteFooter } from "@/components/site-footer"
import { SiteNav } from "@/components/site-nav"

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <SiteNav />
      <main>
        <Hero />
        <MetricsSection />
        <DemoSection />
        <BenchmarkSection />
        <RecoverySection />
        <SecuritySection />
        <EvidenceSection />
        <MilestonesSection />
      </main>
      <SiteFooter />
    </div>
  )
}
