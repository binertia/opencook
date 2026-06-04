import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts'
import type { HealthHistoryEntry } from '@/hooks/useProviders'

interface HealthChartProps {
  data: HealthHistoryEntry[]
}

function formatTime(iso: string) {
  const d = new Date(iso)
  return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

function formatDate(iso: string) {
  const d = new Date(iso)
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

interface ChartRow {
  time: string
  fullDate: string
  latency_ms: number
  error_rate: number
  status: string
}

export function HealthChart({ data }: HealthChartProps) {
  const rows: ChartRow[] = data.map((entry) => ({
    time: formatTime(entry.checked_at),
    fullDate: `${formatDate(entry.checked_at)} ${formatTime(entry.checked_at)}`,
    latency_ms: entry.latency_ms,
    error_rate: entry.error ? 100 : 0,
    status: entry.status,
  }))

  if (rows.length === 0) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        No health data available.
      </div>
    )
  }

  const CustomTooltip = ({ active, payload, label }: {
    active?: boolean
    payload?: Array<{ name: string; value: number; color: string }>
    label?: string
  }) => {
    if (!active || !payload || payload.length === 0) return null

    const row = rows.find((r) => r.time === label)
    return (
      <div className="rounded-md border bg-background p-3 shadow-sm">
        <p className="text-xs text-muted-foreground">{row?.fullDate || label}</p>
        <p className="text-xs capitalize text-muted-foreground mb-1">
          Status: <span className="font-medium">{row?.status}</span>
        </p>
        {payload.map((p) => (
          <p key={p.name} className="text-sm" style={{ color: p.color }}>
            {p.name}: {p.value.toFixed(1)}
            {p.name === 'Latency' ? ' ms' : '%'}
          </p>
        ))}
      </div>
    )
  }

  return (
    <div className="h-72 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <LineChart data={rows} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
          <XAxis
            dataKey="time"
            tick={{ fontSize: 12 }}
            interval="preserveStartEnd"
            minTickGap={30}
          />
          <YAxis
            yAxisId="left"
            tick={{ fontSize: 12 }}
            label={{ value: 'Latency (ms)', angle: -90, position: 'insideLeft', fontSize: 12 }}
          />
          <YAxis
            yAxisId="right"
            orientation="right"
            domain={[0, 100]}
            tick={{ fontSize: 12 }}
            label={{ value: 'Error Rate (%)', angle: 90, position: 'insideRight', fontSize: 12 }}
          />
          <Tooltip content={<CustomTooltip />} />
          <Legend wrapperStyle={{ fontSize: 12 }} />
          <Line
            yAxisId="left"
            type="monotone"
            dataKey="latency_ms"
            name="Latency"
            stroke="#2563eb"
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4 }}
          />
          <Line
            yAxisId="right"
            type="step"
            dataKey="error_rate"
            name="Error Rate"
            stroke="#dc2626"
            strokeWidth={2}
            dot={false}
            activeDot={{ r: 4 }}
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}
