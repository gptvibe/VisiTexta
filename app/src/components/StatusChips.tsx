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
    <div className="status-strip">
      {items.map((item) => (
        <div
          key={item.label}
          className={`status-card${item.wide ? ' wide' : ''}`}
        >
          <span>{item.label}</span>
          <strong>{item.value}</strong>
        </div>
      ))}
    </div>
  )
}
