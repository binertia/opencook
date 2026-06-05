import { useState } from 'react'
import { Link } from 'react-router-dom'
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  AreaChart,
  Area,
  Legend,
} from 'recharts'
import {
  ArrowLeft,
  FileSpreadsheet,
  FileJson,
  Zap,
  MessageSquare,
  MessagesSquare,
} from 'lucide-react'
import { useAnalytics } from '@/hooks/useAnalytics'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'

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

export default function TokenUsagePage() {
  const [timeRange, setTimeRange] = useState('30d')
  const { data, isLoading } = useAnalytics(timeRange)

  const avgTokensPerRequest =
    data && data.total_requests > 0
      ? (data.total_tokens / data.total_requests).toFixed(0)
      : '0'

  const tokenDistribution =
    data?.time_series.map((point) => ({
      date: new Date(point.timestamp).toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
      }),
      prompt: point.prompt_tokens,
      completion: point.completion_tokens,
    })) || []

  const tokenTimeSeries =
    data?.time_series.map((point) => ({
      date: new Date(point.timestamp).toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
      }),
      total: point.tokens,
      prompt: point.prompt_tokens,
      completion: point.completion_tokens,
    })) || []

  const tokensByModel =
    data?.by_model
      .map((item) => ({
        model: item.value,
        tokens: item.tokens,
        prompt: item.prompt_tokens,
        completion: item.completion_tokens,
      }))
      .sort((a, b) => b.tokens - a.tokens) || []

  const handleExportCsv = () => {
    if (!data) return
    exportToCsv(
      `token-usage-${timeRange}.csv`,
      data.time_series.map((p) => ({
        timestamp: p.timestamp,
        requests: p.requests,
        prompt_tokens: p.prompt_tokens,
        completion_tokens: p.completion_tokens,
        total_tokens: p.tokens,
      }))
    )
  }

  const handleExportJson = () => {
    if (!data) return
    exportToJson(`token-usage-${timeRange}.json`, {
      range: timeRange,
      total_prompt_tokens: data.prompt_tokens,
      total_completion_tokens: data.completion_tokens,
      total_tokens: data.total_tokens,
      avg_tokens_per_request: Number(avgTokensPerRequest),
      time_series: data.time_series,
      by_model: data.by_model,
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
            <h1 className="text-3xl font-bold tracking-tight">Token Usage</h1>
            <p className="text-muted-foreground">
              Detailed token consumption analytics.
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
      <Tabs value="tokens" className="w-full">
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
      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-4">
        <KpiCard
          title="Avg Tokens / Request"
          value={avgTokensPerRequest}
          icon={Zap}
          iconColor="text-blue-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Prompt Tokens"
          value={
            data
              ? new Intl.NumberFormat('en-US').format(data.prompt_tokens)
              : '0'
          }
          icon={MessageSquare}
          iconColor="text-yellow-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Completion Tokens"
          value={
            data
              ? new Intl.NumberFormat('en-US').format(data.completion_tokens)
              : '0'
          }
          icon={MessagesSquare}
          iconColor="text-green-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Tokens"
          value={
            data
              ? new Intl.NumberFormat('en-US').format(data.total_tokens)
              : '0'
          }
          icon={Zap}
          iconColor="text-purple-500"
          isLoading={isLoading}
        />
      </div>

      {/* Charts */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Token Distribution Over Time</CardTitle>
            <CardDescription>Prompt vs completion tokens per period.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-72 w-full" />
            ) : tokenDistribution.length === 0 ? (
              <EmptyState />
            ) : (
              <div className="h-72 w-full">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={tokenDistribution}>
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
                        name === 'prompt' ? 'Prompt' : 'Completion',
                      ]}
                    />
                    <Legend />
                    <Bar dataKey="prompt" stackId="a" fill="#3b82f6" name="Prompt" />
                    <Bar dataKey="completion" stackId="a" fill="#10b981" name="Completion" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Token Usage Over Time</CardTitle>
            <CardDescription>Total tokens trend.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-72 w-full" />
            ) : tokenTimeSeries.length === 0 ? (
              <EmptyState />
            ) : (
              <div className="h-72 w-full">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={tokenTimeSeries}>
                    <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                    <XAxis dataKey="date" tick={{ fontSize: 12 }} />
                    <YAxis tick={{ fontSize: 12 }} />
                    <Tooltip
                      contentStyle={{
                        backgroundColor: 'hsl(var(--background))',
                        borderColor: 'hsl(var(--border))',
                      }}
                      formatter={(value: number) =>
                        new Intl.NumberFormat('en-US').format(value)
                      }
                    />
                    <Legend />
                    <Area
                      type="monotone"
                      dataKey="total"
                      stroke="#8b5cf6"
                      fill="#8b5cf6"
                      fillOpacity={0.2}
                      name="Total Tokens"
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      {/* Tokens by Model */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Tokens by Model</CardTitle>
            <CardDescription>Horizontal breakdown of prompt vs completion tokens.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <Skeleton className="h-72 w-full" />
            ) : tokensByModel.length === 0 ? (
              <EmptyState />
            ) : (
              <div className="h-72 w-full">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart
                    data={tokensByModel.slice(0, 8)}
                    layout="vertical"
                    margin={{ top: 5, right: 20, left: 40, bottom: 5 }}
                  >
                    <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
                    <XAxis type="number" tick={{ fontSize: 12 }} />
                    <YAxis
                      dataKey="model"
                      type="category"
                      tick={{ fontSize: 11 }}
                      width={120}
                    />
                    <Tooltip
                      contentStyle={{
                        backgroundColor: 'hsl(var(--background))',
                        borderColor: 'hsl(var(--border))',
                      }}
                      formatter={(value: number, name: string) => [
                        new Intl.NumberFormat('en-US').format(value),
                        name === 'prompt' ? 'Prompt' : 'Completion',
                      ]}
                    />
                    <Legend />
                    <Bar dataKey="prompt" stackId="a" fill="#3b82f6" name="Prompt" />
                    <Bar dataKey="completion" stackId="a" fill="#10b981" name="Completion" />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Model Details</CardTitle>
            <CardDescription>Token counts per model.</CardDescription>
          </CardHeader>
          <CardContent>
            {isLoading ? (
              <div className="space-y-3">
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
                <Skeleton className="h-8 w-full" />
              </div>
            ) : tokensByModel.length === 0 ? (
              <EmptyState />
            ) : (
              <div className="space-y-3">
                {tokensByModel.map((item) => (
                  <div
                    key={item.model}
                    className="flex items-center justify-between rounded-md border p-3"
                  >
                    <div className="min-w-0 flex-1">
                      <p className="text-sm font-medium truncate">{item.model}</p>
                      <div className="mt-1 flex items-center gap-2 text-xs text-muted-foreground">
                        <span className="text-blue-500">
                          {new Intl.NumberFormat('en-US').format(item.prompt)} prompt
                        </span>
                        <span>·</span>
                        <span className="text-green-500">
                          {new Intl.NumberFormat('en-US').format(item.completion)} completion
                        </span>
                      </div>
                    </div>
                    <div className="text-right ml-4">
                      <p className="text-sm font-medium">
                        {new Intl.NumberFormat('en-US').format(item.tokens)} tokens
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

function EmptyState() {
  return (
    <div className="flex h-64 items-center justify-center text-muted-foreground">
      No data available for the selected time range.
    </div>
  )
}
