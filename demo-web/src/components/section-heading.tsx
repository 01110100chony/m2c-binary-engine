export function SectionHeading({ eyebrow, title, description }: { eyebrow?: string; title: string; description?: string }) {
  return (
    <div className="mb-10 flex flex-col gap-3">
      {eyebrow ? <span className="font-mono text-sm text-primary">{eyebrow}</span> : null}
      <h2 className="max-w-3xl text-balance text-3xl font-semibold tracking-[-0.025em] text-foreground md:text-4xl">{title}</h2>
      {description ? <p className="max-w-2xl text-pretty text-base leading-7 text-muted-foreground">{description}</p> : null}
    </div>
  )
}
