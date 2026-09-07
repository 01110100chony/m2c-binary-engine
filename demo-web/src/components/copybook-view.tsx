import { Fragment } from "react"

const KEYWORDS = new Set(["PIC", "COMP-3", "COMP", "USAGE", "VALUE"])

function highlightToken(token: string, key: number) {
  if (/^\d\d$/.test(token)) return <span key={key} className="text-primary">{token}</span>
  if (KEYWORDS.has(token)) return <span key={key} className="text-warn">{token}</span>
  if (/^(9|X|S|V)[9XSV()0-9]*$/.test(token) && /[()]/.test(token)) return <span key={key} className="text-pass">{token}</span>
  if (/-/.test(token) && /[A-Z]/.test(token)) return <span key={key} className="text-foreground">{token}</span>
  return <span key={key} className="text-muted-foreground">{token}</span>
}

export function CopybookView({ code }: { code: string }) {
  return (
    <pre className="overflow-x-auto font-mono text-xs leading-relaxed"><code>{code.split("\n").map((line, lineIndex) => (
      <Fragment key={lineIndex}>
        <span className="mr-3 inline-block w-4 select-none text-right text-border">{lineIndex + 1}</span>
        {line.split(/(\s+)/).map((part, partIndex) => {
          if (/^\s+$/.test(part)) return <Fragment key={partIndex}>{part}</Fragment>
          const hasDot = part.endsWith(".")
          const token = hasDot ? part.slice(0, -1) : part
          return <Fragment key={partIndex}>{highlightToken(token, partIndex)}{hasDot ? <span className="text-muted-foreground">.</span> : null}</Fragment>
        })}
        {"\n"}
      </Fragment>
    ))}</code></pre>
  )
}
