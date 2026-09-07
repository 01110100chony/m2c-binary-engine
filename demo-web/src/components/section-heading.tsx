export function SectionHeading({ index, title, description }: { index: string; title: string; description?: string }) {
  return (
    <div className="mb-8 flex flex-col gap-2">
      <span className="font-mono text-xs tracking-widest text-primary/80">{index}</span>
      <h2 className="text-balance text-xl font-semibold tracking-tight text-foreground md:text-2xl">{title}</h2>
      {description ? <p className="max-w-2xl text-pretty text-sm leading-relaxed text-muted-foreground">{description}</p> : null}
    </div>
  )
}
