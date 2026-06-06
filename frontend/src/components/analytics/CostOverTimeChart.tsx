import {
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
  Area,
  ComposedChart,
} from 'recharts'
import type { TimeSeriesPoint } from '@/hooks/useAnalytics'

interface CostOverTimeChartProps {
  data: TimeSeriesPoint[]
}

function formatTime(iso: string) {
  const d = new Date(iso)
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

export function CostOverTimeChart({ data }: CostOverTimeChartProps) {
  const rows = data.map((point) => ({
    time: formatTime(point.timestamp),
    cost: point.cost_usd,
    requests: point.requests,
    tokens: point.tokens,
  }))

  if (rows.length === 0) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        No data available.
      </div>
    )
  }

  return (
    <div className="h-72 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <ComposedChart data={rows} margin={{ top: 5, right: 20, left: 10, bottom: 5 }}>
          <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
          <XAxis dataKey="time" tick={{ fontSize: 12 }} />
          <YAxis
            yAxisId="left"
            tick={{ fontSize: 12 }}
            label={{ value: 'Cost ($)', angle: -90, position: 'insideLeft', fontSize: 12 }}
          />
          <YAxis
            yAxisId="right"
            orientation="right"
            tick={{ fontSize: 12 }}
            label={{ value: 'Requests', angle: 90, position: 'insideRight', fontSize: 12 }}
          />
          <Tooltip
            contentStyle={{ fontSize: 12 }}
            formatter={(value: number, name: string) => {
              if (name === 'Cost') return [`$${value.toFixed(4)}`, name]
              return [value.toLocaleString(), name]
            }}
          />
          <Legend wrapperStyle={{ fontSize: 12 }} />
          <Area
            yAxisId="right"
            type="monotone"
            dataKey="requests"
            name="Requests"
            fill="#2563eb"
            fillOpacity={0.1}
            stroke="#2563eb"
            strokeWidth={2}
            dot={false}
          />
          <Line
            yAxisId="left"
            type="monotone"
            dataKey="cost"
            name="Cost"
            stroke="#16a34a"
            strokeWidth={2}
            dot={false}
          />
        </ComposedChart>
      </ResponsiveContainer>
    </div>
  )
}
