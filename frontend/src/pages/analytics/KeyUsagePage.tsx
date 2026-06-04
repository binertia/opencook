import { useState } from 'react'
import { Link } from 'react-router-dom'
import { ArrowLeft, FileSpreadsheet, FileJson } from 'lucide-react'
import { useKeyUsage } from '@/hooks/useKeyUsage'
import { KeyUsageTable } from '@/components/analytics/KeyUsageTable'
import { KeyDetailPanel } from '@/components/analytics/KeyDetailPanel'
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

export default function KeyUsagePage() {
  const [timeRange, setTimeRange] = useState('30d')
  const [selectedKeyId, setSelectedKeyId] = useState<string | null>(null)
  const { data, isLoading } = useKeyUsage(timeRange)

  const totalRequests = data?.reduce((s, k) => s + k.requests, 0) || 0
  const totalTokens = data?.reduce((s, k) => s + k.tokens, 0) || 0
  const totalCost = data?.reduce((s, k) => s + k.cost_usd, 0) || 0

  const handleExportCsv = () => {
    if (!data) return
    exportToCsv(
      `key-usage-${timeRange}.csv`,
      data.map((k) => ({
        key_name: k.key_name,
        key_prefix: k.key_prefix,
        status: k.key_status,
        requests: k.requests,
        tokens: k.tokens,
        cost_usd: k.cost_usd.toFixed(4),
        avg_latency_ms: k.avg_latency_ms,
      }))
    )
  }

  const handleExportJson = () => {
    if (!data) return
    exportToJson(`key-usage-${timeRange}.json`, {
      range: timeRange,
      total_requests: totalRequests,
      total_tokens: totalTokens,
      total_cost_usd: totalCost,
      keys: data,
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
            <h1 className="text-3xl font-bold tracking-tight">API Key Usage</h1>
            <p className="text-muted-foreground">
              Per-key usage, cost, and token consumption.
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
      <Tabs value="keys" className="w-full">
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
        </TabsList>
      </Tabs>

      {/* KPI Cards */}
      <div className="grid gap-4 md:grid-cols-3">
        <KpiCard
          title="Total Requests"
          value={new Intl.NumberFormat('en-US').format(totalRequests)}
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Tokens"
          value={new Intl.NumberFormat('en-US').format(totalTokens)}
          isLoading={isLoading}
        />
        <KpiCard
          title="Total Cost"
          value={
            new Intl.NumberFormat('en-US', {
              style: 'currency',
              currency: 'USD',
            }).format(totalCost)
          }
          isLoading={isLoading}
        />
      </div>

      {/* Table */}
      <Card>
        <CardHeader>
          <CardTitle>Key Usage Breakdown</CardTitle>
          <CardDescription>
            Click a row to see detailed analytics for that key.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-3">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : (
            <KeyUsageTable
              data={data || []}
              onRowClick={(keyId) => setSelectedKeyId(keyId)}
            />
          )}
        </CardContent>
      </Card>

      {/* Drill-down Panel */}
      <KeyDetailPanel
        apiKeyId={selectedKeyId}
        timeRange={timeRange}
        onClose={() => setSelectedKeyId(null)}
      />
    </div>
  )
}

function KpiCard({
  title,
  value,
  isLoading,
}: {
  title: string
  value: string
  isLoading?: boolean
}) {
  return (
    <Card>
      <CardContent className="p-6">
        <div>
          <p className="text-sm text-muted-foreground">{title}</p>
          {isLoading ? (
            <Skeleton className="mt-1 h-8 w-24" />
          ) : (
            <p className="mt-1 text-2xl font-bold">{value}</p>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
