type StatusChip = {
  label: string
  value: string | number
  wide?: boolean
}

type StatusChipsProps = {
  items: StatusChip[]
}

export function StatusChips({ items }: StatusChipsProps) {
  return (
    <div className="topbar-actions">
      {items.map((item) => (
        <div
          key={item.label}
          className={`telemetry-card${item.wide ? ' wide' : ''}`}
        >
          <span>{item.label}</span>
          <strong>{item.value}</strong>
        </div>
      ))}
    </div>
  )
}
