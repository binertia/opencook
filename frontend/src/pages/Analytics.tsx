import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Activity, DollarSign, Zap, AlertTriangle } from 'lucide-react'
import { useAnalytics } from '@/hooks/useAnalytics'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

const TIME_RANGES = [
  { value: 'today', label: 'Today' },
  { value: '7d', label: 'Last 7 Days' },
  { value: '30d', label: 'Last 30 Days' },
]

export default function Analytics() {
  const [timeRange, setTimeRange] = useState('30d')
  const { data, isLoading, error, refetch } = useAnalytics(timeRange)

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Analytics</h1>
          <p className="text-muted-foreground">View usage metrics and cost analytics.</p>
        </div>
        <div className="flex items-center gap-2">
          <Select value={timeRange} onValueChange={setTimeRange}>
            <SelectTrigger className="w-40">
              <SelectValue placeholder="Select range" />
            </SelectTrigger>
            <SelectContent>
              {TIME_RANGES.map((r) => (
                <SelectItem key={r.value} value={r.value}>
                  {r.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      <Tabs value="overview" className="w-full">
        <TabsList>
          <TabsTrigger value="overview" asChild>
            <Link to="/analytics">Overview</Link>
          </TabsTrigger>
          <TabsTrigger value="tokens" asChild>
            <Link to="/analytics/tokens">Tokens</Link>
          </TabsTrigger>
          <TabsTrigger value="cache" asChild>
            <Link to="/analytics/cache">Cache</Link>
          </TabsTrigger>
        </TabsList>
      </Tabs>

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4">
          <p className="text-destructive">
            Failed to load analytics: {error.message}
          </p>
          <Button variant="outline" size="sm" className="mt-2" onClick={() => refetch()}>
            Retry
          </Button>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Total Requests"
          value={data ? new Intl.NumberFormat('en-US').format(data.total_requests) : '0'}
          icon={Activity}
          iconColor="text-blue-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Tokens"
          value={data ? new Intl.NumberFormat('en-US').format(data.total_tokens) : '0'}
          icon={Zap}
          iconColor="text-yellow-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Cost"
          value={
            data
              ? new Intl.NumberFormat('en-US', {
                  style: 'currency',
                  currency: 'USD',
                }).format(data.total_cost_usd)
              : '$0.00'
          }
          icon={DollarSign}
          iconColor="text-green-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Error Rate"
          value={data ? `${data.error_rate.toFixed(1)}%` : '0%'}
          icon={AlertTriangle}
          iconColor="text-red-500"
          isLoading={isLoading}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Cache Hit Rate</CardTitle>
            <CardDescription>
              {data ? `${data.cache_hit_rate.toFixed(1)}%` : '—'}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-8 w-full" />
            ) : (
              <div className="flex items-center gap-4">
                <div className="h-4 flex-1 rounded-full bg-muted overflow-hidden">
                  <div
                    className="h-full bg-green-500 transition-all"
                    style={{ width: `${Math.min(data?.cache_hit_rate || 0, 100)}%` }}
                  />
                </div>
                <span className="text-sm font-medium w-16 text-right">
                  {data?.cache_hit_rate.toFixed(1)}%
                </span>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Avg Latency</CardTitle>
            <CardDescription>
              {data ? `${data.avg_latency_ms.toFixed(0)}ms` : '—'}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-8 w-full" />
            ) : (
              <p className="text-3xl font-bold">{data?.avg_latency_ms.toFixed(0)}ms</p>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>By Model</CardTitle>
            <CardDescription>Top models by request volume.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {isLoading ? (
              <>
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
              </>
            ) : data?.by_model.length === 0 ? (
              <p className="text-sm text-muted-foreground">No data available.</p>
            ) : (
              data?.by_model.map((item) => (
                <div
                  key={item.value}
                  className="flex items-center justify-between rounded-md border p-3"
                >
                  <div>
                    <p className="text-sm font-medium">{item.value}</p>
                    <p className="text-xs text-muted-foreground">
                      {new Intl.NumberFormat('en-US').format(item.tokens)} tokens
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-medium">
                      {new Intl.NumberFormat('en-US').format(item.requests)} reqs
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {new Intl.NumberFormat('en-US', {
                        style: 'currency',
                        currency: 'USD',
                      }).format(item.cost_usd)}
                    </p>
                  </div>
                </div>
              ))
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>By Status</CardTitle>
            <CardDescription>Request breakdown by outcome.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {isLoading ? (
              <>
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
              </>
            ) : data?.by_status.length === 0 ? (
              <p className="text-sm text-muted-foreground">No data available.</p>
            ) : (
              data?.by_status.map((item) => (
                <div
                  key={item.value}
                  className="flex items-center justify-between rounded-md border p-3"
                >
                  <div className="flex items-center gap-3">
                    <Badge
                      variant={
                        item.value === 'success'
                          ? 'default'
                          : item.value === 'cached'
                            ? 'secondary'
                            : 'destructive'
                      }
                    >
                      {item.value}
                    </Badge>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-medium">
                      {new Intl.NumberFormat('en-US').format(item.requests)} reqs
                    </p>
                  </div>
                </div>
              ))
            )}
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Time Series</CardTitle>
          <CardDescription>Requests over time.</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Skeleton className="h-48 w-full" />
          ) : data?.time_series.length === 0 ? (
            <p className="text-sm text-muted-foreground">No data available.</p>
          ) : (
            <div className="space-y-2">
              {data?.time_series.map((point) => (
                <div key={point.timestamp} className="flex items-center gap-4 text-sm">
                  <span className="w-32 text-muted-foreground">
                    {new Date(point.timestamp).toLocaleDateString()}
                  </span>
                  <div className="flex-1 h-4 rounded-full bg-muted overflow-hidden">
                    <div
                      className="h-full bg-blue-500"
                      style={{
                        width: `${Math.min(
                          (point.requests / (data?.time_series.reduce((max, p) => Math.max(max, p.requests), 0) || 1)) * 100,
                          100
                        )}%`,
                      }}
                    />
                  </div>
                  <span className="w-16 text-right font-medium">
                    {point.requests}
                  </span>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}

function KpiCard({
  title,
  value,
  icon: Icon,
  iconColor,
  isLoading,
}: {
  title: string
  value: string
  icon: React.ComponentType<{ className?: string }>
  iconColor: string
  isLoading?: boolean
}) {
  return (
    <Card>
      <CardContent className="p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm text-muted-foreground">{title}</p>
            {isLoading ? (
              <Skeleton className="mt-1 h-8 w-24" />
            ) : (
              <p className="mt-1 text-2xl font-bold">{value}</p>
            )}
          </div>
          <Icon className={`h-8 w-8 ${iconColor}`} />
        </div>
      </CardContent>
    </Card>
  )
}
