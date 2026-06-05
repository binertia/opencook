import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Activity, DollarSign, Zap, AlertTriangle, TrendingUp, Clock, FileSpreadsheet, FileJson } from 'lucide-react'
import { useAnalytics } from '@/hooks/useAnalytics'
import { CostByProviderChart } from '@/components/analytics/CostByProviderChart'
import { CostOverTimeChart } from '@/components/analytics/CostOverTimeChart'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'

const TIME_RANGES = [
  { value: 'today', label: 'Today' },
  { value: '7d', label: 'Last 7 Days' },
  { value: '30d', label: 'Last 30 Days' },
]

function exportToCsv(filename: string, rows: Record<string, string | number>[]) {
  if (rows.length === 0) return
  const headers = Object.keys(rows[0])
  const csv = [
    headers.join(','),
    ...rows.map((row) =>
      headers.map((h) => {
        const val = row[h]
        const str = typeof val === 'number' ? val.toString() : `"${String(val).replace(/"/g, '""')}"`
        return str
      }).join(',')
    ),
  ].join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

function exportToJson(filename: string, data: unknown) {
  const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

export default function Analytics() {
  const [timeRange, setTimeRange] = useState('30d')
  const { data, isLoading, error, refetch } = useAnalytics(timeRange)

  const handleExportCsv = () => {
    if (!data) return
    exportToCsv(
      `analytics-${timeRange}.csv`,
      data.time_series.map((p) => ({
        timestamp: p.timestamp,
        requests: p.requests,
        tokens: p.tokens,
        prompt_tokens: p.prompt_tokens,
        completion_tokens: p.completion_tokens,
        cost_usd: p.cost_usd.toFixed(4),
        latency_ms: p.latency_ms.toFixed(2),
        cache_hits: p.cache_hits,
        cache_misses: p.cache_misses,
      }))
    )
  }

  const handleExportJson = () => {
    if (!data) return
    exportToJson(`analytics-${timeRange}.json`, {
      range: timeRange,
      total_requests: data.total_requests,
      total_tokens: data.total_tokens,
      total_cost_usd: data.total_cost_usd,
      cost_saved_from_cache_usd: data.cost_saved_from_cache_usd,
      avg_latency_ms: data.avg_latency_ms,
      cache_hit_rate: data.cache_hit_rate,
      error_rate: data.error_rate,
      by_model: data.by_model,
      by_provider: data.by_provider,
      by_status: data.by_status,
      time_series: data.time_series,
    })
  }

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
          <Button variant="outline" size="sm" onClick={handleExportCsv}>
            <FileSpreadsheet className="mr-2 h-4 w-4" />
            CSV
          </Button>
          <Button variant="outline" size="sm" onClick={handleExportJson}>
            <FileJson className="mr-2 h-4 w-4" />
            JSON
          </Button>
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
          <TabsTrigger value="keys" asChild>
            <Link to="/analytics/keys">Keys</Link>
          </TabsTrigger>
          <TabsTrigger value="budget" asChild>
            <Link to="/analytics/budget">Budget</Link>
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

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Total Requests"
          value={data ? new Intl.NumberFormat('en-US').format(data.total_requests) : '0'}
          icon={Activity}
          iconColor="text-blue-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Cost"
          value={
            data
              ? new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(
                  data.total_cost_usd
                )
              : '$0.00'
          }
          icon={DollarSign}
          iconColor="text-green-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Cache Hit Rate"
          value={data ? `${data.cache_hit_rate.toFixed(1)}%` : '0%'}
          icon={TrendingUp}
          iconColor="text-purple-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Avg Latency"
          value={data ? `${data.avg_latency_ms.toFixed(0)}ms` : '0ms'}
          icon={Clock}
          iconColor="text-orange-500"
          isLoading={isLoading}
        />
      </div>

      {/* Charts Row 1 */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Cost Over Time</CardTitle>
            <CardDescription>Daily cost and request volume.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? <Skeleton className="h-72 w-full" /> : <CostOverTimeChart data={data?.time_series || []} />}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Cost by Provider</CardTitle>
            <CardDescription>Cost distribution across providers.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? <Skeleton className="h-72 w-full" /> : <CostByProviderChart data={data?.by_provider || []} />}
          </CardContent>
        </Card>
      </div>

      {/* Charts Row 2 */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Cost by Model</CardTitle>
            <CardDescription>Top models by cost.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="space-y-2">
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
              </div>
            ) : data?.by_model.length === 0 ? (
              <p className="text-sm text-muted-foreground">No data available.</p>
            ) : (
              <div className="space-y-3">
                {data?.by_model.map((item) => (
                  <div key={item.value} className="flex items-center justify-between rounded-md border p-3">
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
                        {new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(item.cost_usd)}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>By Status</CardTitle>
            <CardDescription>Request breakdown by outcome.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="space-y-2">
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
              </div>
            ) : data?.by_status.length === 0 ? (
              <p className="text-sm text-muted-foreground">No data available.</p>
            ) : (
              <div className="space-y-3">
                {data?.by_status.map((item) => (
                  <div key={item.value} className="flex items-center justify-between rounded-md border p-3">
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
                ))}
              </div>
            )}
          </CardContent>
        </Card>
      </div>
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
