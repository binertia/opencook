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
import {
  KeyRound,
  Activity,
  DollarSign,
  Zap,
  Clock,
} from 'lucide-react'
import { useKeyDetail } from '@/hooks/useKeyUsage'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

interface KeyDetailPanelProps {
  apiKeyId: string | null
  timeRange: string
  onClose: () => void
}

export function KeyDetailPanel({ apiKeyId, timeRange, onClose }: KeyDetailPanelProps) {
  const { data, isLoading } = useKeyDetail(apiKeyId || '', timeRange)
  const open = !!apiKeyId

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-3xl max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <KeyRound className="h-5 w-5" />
            API Key Details
          </DialogTitle>
          <DialogDescription>
            Usage analytics for the selected API key.
          </DialogDescription>
        </DialogHeader>

        {isLoading || !data ? (
          <div className="space-y-4">
            <Skeleton className="h-8 w-48" />
            <div className="grid grid-cols-2 gap-4">
              <Skeleton className="h-24 w-full" />
              <Skeleton className="h-24 w-full" />
            </div>
            <Skeleton className="h-64 w-full" />
          </div>
        ) : (
          <div className="space-y-6">
            {/* Key Info */}
            <div className="flex items-center justify-between">
              <div>
                <h3 className="text-lg font-semibold">{data.key.key_name}</h3>
                <p className="text-sm text-muted-foreground font-mono">
                  {data.key.key_prefix}…
                </p>
              </div>
              <div className="flex items-center gap-2">
                <Badge
                  variant={data.key.key_status === 'active' ? 'default' : 'secondary'}
                >
                  {data.key.key_status}
                </Badge>
              </div>
            </div>

            {/* KPI Cards */}
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <KpiCard
                title="Requests"
                value={new Intl.NumberFormat('en-US').format(data.key.requests)}
                icon={Activity}
                iconColor="text-blue-500"
              />
              <KpiCard
                title="Tokens"
                value={new Intl.NumberFormat('en-US').format(data.key.tokens)}
                icon={Zap}
                iconColor="text-yellow-500"
              />
              <KpiCard
                title="Cost"
                value={new Intl.NumberFormat('en-US', {
                  style: 'currency',
                  currency: 'USD',
                }).format(data.key.cost_usd)}
                icon={DollarSign}
                iconColor="text-green-500"
              />
              <KpiCard
                title="Avg Latency"
                value={
                  data.key.avg_latency_ms > 0
                    ? `${data.key.avg_latency_ms}ms`
                    : '—'
                }
                icon={Clock}
                iconColor="text-purple-500"
              />
            </div>

            {/* Usage Over Time Chart */}
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Usage Over Time</CardTitle>
                <CardDescription>Daily requests and cost.</CardDescription>
              </CardHeader>
              <CardContent>
                {data.time_series.length === 0 ? (
                  <EmptyState />
                ) : (
                  <div className="h-64 w-full">
                    <ResponsiveContainer width="100%" height="100%">
                      <LineChart data={data.time_series}>
                        <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                        <XAxis
                          dataKey="period_start"
                          tickFormatter={(v: string) =>
                            new Date(v).toLocaleDateString(undefined, {
                              month: 'short',
                              day: 'numeric',
                            })
                          }
                          tick={{ fontSize: 12 }}
                        />
                        <YAxis yAxisId="left" tick={{ fontSize: 12 }} />
                        <YAxis
                          yAxisId="right"
                          orientation="right"
                          tick={{ fontSize: 12 }}
                          tickFormatter={(v: number) => `$${v.toFixed(2)}`}
                        />
                        <Tooltip
                          contentStyle={{
                            backgroundColor: 'hsl(var(--background))',
                            borderColor: 'hsl(var(--border))',
                          }}
                          formatter={(value: number, name: string) => {
                            if (name === 'Cost') {
                              return [new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(value), name]
                            }
                            return [new Intl.NumberFormat('en-US').format(value), name]
                          }}
                          labelFormatter={(label: string) =>
                            new Date(label).toLocaleDateString()
                          }
                        />
                        <Legend />
                        <Line
                          yAxisId="left"
                          type="monotone"
                          dataKey="requests"
                          stroke="#3b82f6"
                          strokeWidth={2}
                          dot={false}
                          name="Requests"
                        />
                        <Line
                          yAxisId="right"
                          type="monotone"
                          dataKey="cost_usd"
                          stroke="#10b981"
                          strokeWidth={2}
                          dot={false}
                          name="Cost"
                        />
                      </LineChart>
                    </ResponsiveContainer>
                  </div>
                )}
              </CardContent>
            </Card>

            {/* Top Models */}
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Top Models</CardTitle>
                <CardDescription>Models used by this key.</CardDescription>
              </CardHeader>
              <CardContent>
                {data.top_models.length === 0 ? (
                  <EmptyState />
                ) : (
                  <div className="space-y-2">
                    {data.top_models.map((m) => (
                      <div
                        key={m.model_id}
                        className="flex items-center justify-between rounded-md border p-3"
                      >
                        <div>
                          <p className="text-sm font-medium font-mono">{m.model_id}</p>
                          <p className="text-xs text-muted-foreground">
                            {new Intl.NumberFormat('en-US').format(m.requests)} requests ·{' '}
                            {new Intl.NumberFormat('en-US').format(m.tokens)} tokens
                          </p>
                        </div>
                        <div className="text-right">
                          <p className="text-sm font-medium">
                            {new Intl.NumberFormat('en-US', {
                              style: 'currency',
                              currency: 'USD',
                            }).format(m.cost_usd)}
                          </p>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}

function KpiCard({
  title,
  value,
  icon: Icon,
  iconColor,
}: {
  title: string
  value: string
  icon: React.ComponentType<{ className?: string }>
  iconColor: string
}) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-xs text-muted-foreground">{title}</p>
            <p className="mt-1 text-lg font-bold">{value}</p>
          </div>
          <Icon className={`h-6 w-6 ${iconColor}`} />
        </div>
      </CardContent>
    </Card>
  )
}

function EmptyState() {
  return (
    <div className="flex h-32 items-center justify-center text-sm text-muted-foreground">
      No data available.
    </div>
  )
}
