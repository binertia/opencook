import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip, Legend } from 'recharts'
import type { BreakdownItem } from '@/hooks/useAnalytics'

const COLORS = ['#2563eb', '#16a34a', '#dc2626', '#9333ea', '#ea580c', '#0891b2', '#4f46e5', '#db2777']

interface CostByProviderChartProps {
  data: BreakdownItem[]
}

export function CostByProviderChart({ data }: CostByProviderChartProps) {
  const chartData = data.map((item) => ({
    name: item.value,
    cost: item.cost_usd,
    requests: item.requests,
  }))

  const totalCost = chartData.reduce((sum, d) => sum + d.cost, 0)

  if (chartData.length === 0) {
    return (
      <div className="flex h-64 items-center justify-center text-muted-foreground">
        No data available.
      </div>
    )
  }

  return (
    <div className="h-72 w-full">
      <ResponsiveContainer width="100%" height="100%">
        <PieChart>
          <Pie
            data={chartData}
            cx="50%"
            cy="50%"
            innerRadius={60}
            outerRadius={100}
            paddingAngle={2}
            dataKey="cost"
            nameKey="name"
          >
            {chartData.map((_, index) => (
              <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
            ))}
          </Pie>
          <Tooltip
            formatter={(value: number, _name: string, props: unknown) => {
              const p = props as { payload: { name: string; requests: number } }
              const pct = totalCost > 0 ? ((value / totalCost) * 100).toFixed(1) : '0.0'
              return [`$${value.toFixed(4)} (${pct}%) — ${p.payload.requests} reqs`, p.payload.name]
            }}
          />
          <Legend wrapperStyle={{ fontSize: 12 }} />
        </PieChart>
      </ResponsiveContainer>
    </div>
  )
}
