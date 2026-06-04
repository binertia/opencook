import { useMemo, useState } from 'react'
import { ArrowUpDown, RefreshCw, Search } from 'lucide-react'
import { useProviders, useProviderHealth, useTriggerHealthCheck } from '@/hooks/useProviders'
import type { Provider } from '@/hooks/useProviders'
import { HealthIndicator } from '@/components/providers/HealthIndicator'
import { AddProviderWizard } from '@/components/providers/AddProviderWizard'
import { EditProviderModal } from '@/components/providers/EditProviderModal'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Skeleton } from '@/components/ui/skeleton'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

type SortKey = 'name' | 'kind' | 'status' | 'latency' | 'error_rate'
type SortDir = 'asc' | 'desc'

const LATENCY_THRESHOLDS = {
  good: 500,
  warning: 2000,
}

const ERROR_RATE_THRESHOLDS = {
  good: 0.01,
  warning: 0.05,
}

function LatencyCell({ latencyMs }: { latencyMs?: number }) {
  if (latencyMs === undefined) return <span className="text-muted-foreground">—</span>

  let color = 'text-green-600 dark:text-green-400'
  if (latencyMs >= LATENCY_THRESHOLDS.warning) {
    color = 'text-red-600 dark:text-red-400'
  } else if (latencyMs >= LATENCY_THRESHOLDS.good) {
    color = 'text-yellow-600 dark:text-yellow-400'
  }

  return <span className={cn('font-medium', color)}>{latencyMs}ms</span>
}

function ErrorRateCell({ errorRate }: { errorRate?: number }) {
  if (errorRate === undefined) return <span className="text-muted-foreground">—</span>

  let color = 'text-green-600 dark:text-green-400'
  if (errorRate >= ERROR_RATE_THRESHOLDS.warning) {
    color = 'text-red-600 dark:text-red-400'
  } else if (errorRate >= ERROR_RATE_THRESHOLDS.good) {
    color = 'text-yellow-600 dark:text-yellow-400'
  }

  return <span className={cn('font-medium', color)}>{(errorRate * 100).toFixed(2)}%</span>
}

function ProviderHealthRow({ providerId }: { providerId: string }) {
  const { data: health } = useProviderHealth(providerId)
  const triggerCheck = useTriggerHealthCheck()

  return (
    <>
      <TableCell>
        <HealthIndicator health={health} />
      </TableCell>
      <TableCell>
        <LatencyCell latencyMs={health?.latency_ms} />
      </TableCell>
      <TableCell>
        <ErrorRateCell errorRate={health?.error_rate} />
      </TableCell>
      <TableCell>
        <Button
          variant="ghost"
          size="icon"
          onClick={() => triggerCheck.mutate(providerId)}
          disabled={triggerCheck.isPending}
          aria-label="Check health"
        >
          <RefreshCw className={cn('h-4 w-4', triggerCheck.isPending && 'animate-spin')} />
        </Button>
      </TableCell>
    </>
  )
}

export default function ProvidersPage() {
  const { data, isLoading, error, refetch } = useProviders()
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | 'active' | 'inactive'>('all')
  const [sort, setSort] = useState<{ key: SortKey; dir: SortDir }>({
    key: 'name',
    dir: 'asc',
  })
  const [editProvider, setEditProvider] = useState<Provider | null>(null)

  const filtered = useMemo(() => {
    let result = data?.data || []

    if (search) {
      const s = search.toLowerCase()
      result = result.filter(
        (p) =>
          p.name.toLowerCase().includes(s) || p.kind.toLowerCase().includes(s)
      )
    }

    if (statusFilter !== 'all') {
      result = result.filter((p) => p.status === statusFilter)
    }

    return result
  }, [data?.data, search, statusFilter])

  const toggleSort = (key: SortKey) => {
    setSort((prev) => ({
      key,
      dir: prev.key === key && prev.dir === 'asc' ? 'desc' : 'asc',
    }))
  }

  const SortIcon = ({ column }: { column: SortKey }) => (
    <ArrowUpDown
      className={cn(
        'ml-1 h-3 w-3 inline cursor-pointer',
        sort.key === column ? 'text-primary' : 'text-muted-foreground'
      )}
      onClick={() => toggleSort(column)}
    />
  )

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Providers</h1>
          <p className="text-muted-foreground">
            Manage and monitor AI provider configurations.
          </p>
        </div>
        <AddProviderWizard />
      </div>

      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search providers..."
            className="pl-8"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <div className="flex gap-2">
          {(['all', 'active', 'inactive'] as const).map((s) => (
            <Button
              key={s}
              variant={statusFilter === s ? 'default' : 'outline'}
              size="sm"
              onClick={() => setStatusFilter(s)}
            >
              {s.charAt(0).toUpperCase() + s.slice(1)}
            </Button>
          ))}
        </div>
      </div>

      {isLoading && (
        <div className="space-y-2">
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
          <Skeleton className="h-10 w-full" />
        </div>
      )}

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          <p>Failed to load providers: {error.message}</p>
          <Button variant="outline" size="sm" className="mt-2" onClick={() => refetch()}>
            Retry
          </Button>
        </div>
      )}

      {!isLoading && !error && (
        <Card>
          <CardContent className="p-0">
            <div className="rounded-md border-0">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>
                      Name <SortIcon column="name" />
                    </TableHead>
                    <TableHead>
                      Kind <SortIcon column="kind" />
                    </TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Health</TableHead>
                    <TableHead>
                      Latency <SortIcon column="latency" />
                    </TableHead>
                    <TableHead>
                      Error Rate <SortIcon column="error_rate" />
                    </TableHead>
                    <TableHead className="w-12" />
                    <TableHead className="w-20">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filtered.length === 0 ? (
                    <TableRow>
                      <TableCell
                        colSpan={8}
                        className="text-center text-muted-foreground"
                      >
                        No providers found.
                      </TableCell>
                    </TableRow>
                  ) : (
                    filtered.map((provider) => (
                      <TableRow key={provider.id}>
                        <TableCell className="font-medium">{provider.name}</TableCell>
                        <TableCell className="capitalize">{provider.kind}</TableCell>
                        <TableCell>
                          <Badge
                            variant={provider.status === 'active' ? 'default' : 'secondary'}
                          >
                            {provider.status}
                          </Badge>
                        </TableCell>
                        <ProviderHealthRow providerId={provider.id} />
                        <TableCell>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setEditProvider(provider)}
                          >
                            Edit
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>
          </CardContent>
        </Card>
      )}

      <EditProviderModal
        provider={editProvider}
        open={!!editProvider}
        onOpenChange={() => setEditProvider(null)}
      />
    </div>
  )
}
