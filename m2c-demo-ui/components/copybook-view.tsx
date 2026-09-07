import { Fragment } from "react"

const KEYWORDS = new Set(["PIC", "COMP-3", "COMP", "USAGE", "VALUE", "OCCURS", "REDEFINES"])

function highlightToken(token: string, key: number) {
  if (/^\d\d$/.test(token)) {
    // level number (01, 05, ...)
    return (
      <span key={key} className="text-primary">
        {token}
      </span>
    )
  }
  if (KEYWORDS.has(token)) {
    return (
      <span key={key} className="text-warn">
        {token}
      </span>
    )
  }
  if (/^(9|X|S|V|A)[9XSVA()0-9]*$/.test(token) && /[()]/.test(token)) {
    // picture clause like 9(6), S9(7)V99, X(20)
    return (
      <span key={key} className="text-pass">
        {token}
      </span>
    )
  }
  if (/-/.test(token) && /[A-Z]/.test(token)) {
    // field / record names
    return (
      <span key={key} className="text-foreground">
        {token}
      </span>
    )
  }
  return (
    <span key={key} className="text-muted-foreground">
      {token}
    </span>
  )
}

export function CopybookView({ code }: { code: string }) {
  const lines = code.split("\n")
  return (
    <pre className="overflow-x-auto font-mono text-xs leading-relaxed">
      <code>
        {lines.map((line, li) => {
          const parts = line.split(/(\s+)/)
          return (
            <Fragment key={li}>
              <span className="mr-3 inline-block w-4 select-none text-right text-border">
                {li + 1}
              </span>
              {parts.map((p, pi) => {
                if (/^\s+$/.test(p)) return <Fragment key={pi}>{p}</Fragment>
                const hasDot = p.endsWith(".")
                const token = hasDot ? p.slice(0, -1) : p
                return (
                  <Fragment key={pi}>
                    {highlightToken(token, pi)}
                    {hasDot ? <span className="text-muted-foreground">.</span> : null}
                  </Fragment>
                )
              })}
              {"\n"}
            </Fragment>
          )
        })}
      </code>
    </pre>
  )
}
