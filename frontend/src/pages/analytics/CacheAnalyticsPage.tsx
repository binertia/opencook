import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Legend,
} from 'recharts'
import {
  ArrowLeft,
  FileSpreadsheet,
  FileJson,
  Database,
  DollarSign,
  TrendingUp,
  AlertTriangle,
} from 'lucide-react'
import { useAnalytics } from '@/hooks/useAnalytics'
import { useCacheStats } from '@/hooks/useCacheStats'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

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

export default function CacheAnalyticsPage() {
  const [timeRange, setTimeRange] = useState('30d')
  const { data, isLoading } = useAnalytics(timeRange)
  const { data: cacheStats, isLoading: cacheStatsLoading } = useCacheStats(timeRange)

  const cacheHitRate = data?.cache_hit_rate ?? 0
  const costSaved = data?.cost_saved_from_cache_usd ?? 0
  const totalCacheHits =
    data?.time_series.reduce((sum, p) => sum + p.cache_hits, 0) ?? 0
  const totalCacheMisses =
    data?.time_series.reduce((sum, p) => sum + p.cache_misses, 0) ?? 0
  const entryCount = cacheStats?.entry_count ?? 0

  const cacheTimeSeries =
    data?.time_series.map((point) => ({
      date: new Date(point.timestamp).toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
      }),
      hits: point.cache_hits,
      misses: point.cache_misses,
    })) || []

  const hitRateColor =
    cacheHitRate >= 20 ? 'bg-green-500' : cacheHitRate >= 10 ? 'bg-yellow-500' : 'bg-red-500'

  const hitRateTextColor =
    cacheHitRate >= 20 ? 'text-green-500' : cacheHitRate >= 10 ? 'text-yellow-500' : 'text-red-500'

  const handleExportCsv = () => {
    if (!data) return
    exportToCsv(
      `cache-analytics-${timeRange}.csv`,
      data.time_series.map((p) => ({
        timestamp: p.timestamp,
        cache_hits: p.cache_hits,
        cache_misses: p.cache_misses,
      }))
    )
  }

  const handleExportJson = () => {
    if (!data) return
    exportToJson(`cache-analytics-${timeRange}.json`, {
      range: timeRange,
      cache_hit_rate: data.cache_hit_rate,
      cost_saved_from_cache_usd: data.cost_saved_from_cache_usd,
      total_cache_hits: totalCacheHits,
      total_cache_misses: totalCacheMisses,
      time_series: data.time_series.map((p) => ({
        timestamp: p.timestamp,
        cache_hits: p.cache_hits,
        cache_misses: p.cache_misses,
      })),
      top_cached_models: data.top_cached_models,
    })
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="sm" asChild>
            <Link to="/analytics">
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Link>
          </Button>
          <div>
            <h1 className="text-3xl font-bold tracking-tight">Cache Analytics</h1>
            <p className="text-muted-foreground">
              Cache performance and cost savings.
            </p>
          </div>
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

      {/* Analytics Tabs */}
      <Tabs value="cache" className="w-full">
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

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-3 lg:grid-cols-5">
        <KpiCard
          title="Cache Hit Rate"
          value={`${cacheHitRate.toFixed(1)}%`}
          icon={TrendingUp}
          iconColor={hitRateTextColor}
          isLoading={isLoading}
        />
        <KpiCard
          title="Cost Saved"
          value={
            data
              ? new Intl.NumberFormat('en-US', {
                  style: 'currency',
                  currency: 'USD',
                }).format(costSaved)
              : '$0.00'
          }
          icon={DollarSign}
          iconColor="text-green-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Cache Hits"
          value={new Intl.NumberFormat('en-US').format(totalCacheHits)}
          icon={Database}
          iconColor="text-blue-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Cache Misses"
          value={new Intl.NumberFormat('en-US').format(totalCacheMisses)}
          icon={AlertTriangle}
          iconColor="text-orange-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Cache Entries"
          value={new Intl.NumberFormat('en-US').format(entryCount)}
          icon={Database}
          iconColor="text-purple-500"
          isLoading={isLoading || cacheStatsLoading}
        />
      </div>

      {/* Hit Rate Gauge */}
      <Card>
        <CardHeader>
          <CardTitle>Cache Hit Rate</CardTitle>
          <CardDescription>
            Percentage of requests served from cache.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Skeleton className="h-16 w-full" />
          ) : (
            <div className="space-y-4">
              <div className="flex items-center gap-4">
                <div className="h-6 flex-1 rounded-full bg-muted overflow-hidden">
                  <div
                    className={`h-full ${hitRateColor} transition-all`}
                    style={{ width: `${Math.min(cacheHitRate, 100)}%` }}
                  />
                </div>
                <span className={`text-2xl font-bold w-20 text-right ${hitRateTextColor}`}>
                  {cacheHitRate.toFixed(1)}%
                </span>
              </div>
              <div className="flex gap-4 text-sm text-muted-foreground">
                <div className="flex items-center gap-2">
                  <div className="h-3 w-3 rounded-full bg-green-500" />
                  <span>Good (&gt;20%)</span>
                </div>
                <div className="flex items-center gap-2">
                  <div className="h-3 w-3 rounded-full bg-yellow-500" />
                  <span>Fair (10-20%)</span>
                </div>
                <div className="flex items-center gap-2">
                  <div className="h-3 w-3 rounded-full bg-red-500" />
                  <span>Poor (&lt;10%)</span>
                </div>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Cache Hit/Miss Over Time */}
      <Card>
        <CardHeader>
          <CardTitle>Cache Hits vs Misses Over Time</CardTitle>
          <CardDescription>Daily cache performance.</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <Skeleton className="h-72 w-full" />
          ) : cacheTimeSeries.length === 0 ? (
            <EmptyState />
          ) : (
            <div className="h-72 w-full">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={cacheTimeSeries}>
                  <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                  <XAxis dataKey="date" tick={{ fontSize: 12 }} />
                  <YAxis tick={{ fontSize: 12 }} />
                  <Tooltip
                    contentStyle={{
                      backgroundColor: 'hsl(var(--background))',
                      borderColor: 'hsl(var(--border))',
                    }}
                    formatter={(value: number, name: string) => [
                      new Intl.NumberFormat('en-US').format(value),
                      name === 'hits' ? 'Hits' : 'Misses',
                    ]}
                  />
                  <Legend />
                  <Area
                    type="monotone"
                    dataKey="hits"
                    stackId="1"
                    stroke="#3b82f6"
                    fill="#3b82f6"
                    fillOpacity={0.6}
                    name="Hits"
                  />
                  <Area
                    type="monotone"
                    dataKey="misses"
                    stackId="1"
                    stroke="#ef4444"
                    fill="#ef4444"
                    fillOpacity={0.6}
                    name="Misses"
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Top Cached Models */}
      <Card>
        <CardHeader>
          <CardTitle>Top Cached Models</CardTitle>
          <CardDescription>Models with the most cache hits and cost savings.</CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-3">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : !data || data.top_cached_models.length === 0 ? (
            <EmptyState />
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Model</TableHead>
                    <TableHead className="text-right">Requests</TableHead>
                    <TableHead className="text-right">Cache Hits</TableHead>
                    <TableHead className="text-right">Hit Rate</TableHead>
                    <TableHead className="text-right">Cost Saved</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.top_cached_models.map((item) => (
                    <TableRow key={item.model}>
                      <TableCell className="font-medium">{item.model}</TableCell>
                      <TableCell className="text-right">
                        {new Intl.NumberFormat('en-US').format(item.requests)}
                      </TableCell>
                      <TableCell className="text-right">
                        {new Intl.NumberFormat('en-US').format(item.cache_hits)}
                      </TableCell>
                      <TableCell className="text-right">
                        <Badge
                          variant={
                            item.cache_hit_rate >= 20
                              ? 'default'
                              : item.cache_hit_rate >= 10
                                ? 'secondary'
                                : 'destructive'
                          }
                        >
                          {item.cache_hit_rate.toFixed(1)}%
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right">
                        {new Intl.NumberFormat('en-US', {
                          style: 'currency',
                          currency: 'USD',
                        }).format(item.cost_saved_usd)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
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

function EmptyState() {
  return (
    <div className="flex h-64 items-center justify-center text-muted-foreground">
      No data available for the selected time range.
    </div>
  )
}
