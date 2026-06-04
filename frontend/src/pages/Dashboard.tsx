import { useState } from 'react'
import { Link } from 'react-router-dom'
import { Activity, DollarSign, KeyRound, Plug, Zap } from 'lucide-react'
// KeyPlug doesn't exist in lucide-react, using KeyRound instead
import { useDashboard } from '@/hooks/useDashboard'
import { KpiCard } from '@/components/dashboard/KpiCard'
import { RecentRequests } from '@/components/dashboard/RecentRequests'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'

const TIME_RANGES = [
  { value: 'today', label: 'Today' },
  { value: '7d', label: 'Last 7 Days' },
  { value: '30d', label: 'Last 30 Days' },
]

function formatChange(value: number): { value: string; direction: 'up' | 'down' | 'neutral' } {
  if (value > 0) return { value: `+${value.toFixed(1)}%`, direction: 'up' as const }
  if (value < 0) return { value: `${value.toFixed(1)}%`, direction: 'down' as const }
  return { value: '0%', direction: 'neutral' as const }
}

export default function Dashboard() {
  const [timeRange, setTimeRange] = useState('today')
  const { data, isLoading, error, refetch } = useDashboard(timeRange)

  const requestsChange = data ? formatChange(data.requests_change) : undefined
  const costChange = data ? formatChange(data.cost_change) : undefined
  const cacheChange = data ? formatChange(data.cache_change) : undefined
  const latencyChange = data ? formatChange(data.latency_change) : undefined

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Dashboard</h1>
          <p className="text-muted-foreground">
            Overview of your AI Gateway usage.
          </p>
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

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4">
          <p className="text-destructive">
            Failed to load dashboard data: {error.message}
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
          change={requestsChange}
          icon={Activity}
          iconColor="text-blue-500"
          borderColor="border-blue-500"
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
          change={costChange}
          icon={DollarSign}
          iconColor="text-green-500"
          borderColor="border-green-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Cache Hit Rate"
          value={data ? `${data.cache_hit_rate.toFixed(1)}%` : '0%'}
          change={cacheChange}
          icon={Zap}
          iconColor="text-yellow-500"
          borderColor="border-yellow-500"
          isLoading={isLoading}
        />
        <KpiCard
          title="Avg Latency"
          value={data ? `${data.avg_latency_ms.toFixed(0)}ms` : '0ms'}
          change={latencyChange}
          icon={Activity}
          iconColor="text-purple-500"
          borderColor="border-purple-500"
          isLoading={isLoading}
        />
      </div>

      <div className="grid gap-6 lg:grid-cols-3">
        <Card className="lg:col-span-2">
          <CardHeader>
            <CardTitle>Recent Requests</CardTitle>
            <CardDescription>Last 10 requests across all providers.</CardDescription>
          </CardHeader>
          <CardContent>
            <RecentRequests
              requests={data?.recent_requests || []}
              isLoading={isLoading}
            />
          </CardContent>
        </Card>

        <div className="space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Active Providers</CardTitle>
              <CardDescription>Health status of configured providers.</CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {isLoading ? (
                <>
                  <Skeleton className="h-10 w-full" />
                  <Skeleton className="h-10 w-full" />
                  <Skeleton className="h-10 w-full" />
                </>
              ) : data?.active_providers.length === 0 ? (
                <p className="text-sm text-muted-foreground">No providers configured.</p>
              ) : (
                data?.active_providers.map((provider) => (
                  <div
                    key={provider.id}
                    className="flex items-center justify-between rounded-md border p-3"
                  >
                    <div className="flex items-center gap-3">
                      <div
                        className={`h-2.5 w-2.5 rounded-full ${
                          provider.status === 'healthy'
                            ? 'bg-green-500'
                            : provider.status === 'degraded'
                              ? 'bg-yellow-500'
                              : 'bg-red-500'
                        }`}
                      />
                      <span className="text-sm font-medium">{provider.name}</span>
                    </div>
                    <Badge
                      variant={
                        provider.status === 'healthy'
                          ? 'default'
                          : provider.status === 'degraded'
                            ? 'secondary'
                            : 'destructive'
                      }
                    >
                      {provider.status}
                    </Badge>
                  </div>
                ))
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Quick Actions</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2">
              <Button asChild className="w-full" variant="outline">
                <Link to="/keys">
                  <KeyRound className="mr-2 h-4 w-4" />
                  Create API Key
                </Link>
              </Button>
              <Button asChild className="w-full" variant="outline">
                <Link to="/providers">
                  <Plug className="mr-2 h-4 w-4" />
                  Add Provider
                </Link>
              </Button>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  )
}
