import { ArchitectureSection } from "@/components/architecture-section"
import { BenchmarkSection } from "@/components/benchmark-section"
import { DemoSection } from "@/components/demo-section"
import { EngineeringDecisions } from "@/components/engineering-decisions"
import { Hero } from "@/components/hero"
import { SiteFooter } from "@/components/site-footer"
import { SiteNav } from "@/components/site-nav"
import { ValidationSection } from "@/components/validation-section"
import { WhyBuiltSection } from "@/components/why-built-section"

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <SiteNav />
      <main>
        <Hero />
        <WhyBuiltSection />
        <DemoSection />
        <ArchitectureSection />
        <BenchmarkSection />
        <ValidationSection />
        <EngineeringDecisions />
      </main>
      <SiteFooter />
    </div>
  )
}
