import { useRequests } from '@/hooks/useRequests'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { Clock, Database, AlertCircle, CheckCircle, XCircle, Zap } from 'lucide-react'

function StatusBadge({ status }: { status: string }) {
  const variants: Record<string, { class: string; icon: React.ElementType }> = {
    success: { class: 'bg-green-100 text-green-800 hover:bg-green-100', icon: CheckCircle },
    error: { class: 'bg-red-100 text-red-800 hover:bg-red-100', icon: XCircle },
    pending: { class: 'bg-yellow-100 text-yellow-800 hover:bg-yellow-100', icon: Clock },
    processing: { class: 'bg-blue-100 text-blue-800 hover:bg-blue-100', icon: Zap },
    cancelled: { class: 'bg-gray-100 text-gray-800 hover:bg-gray-100', icon: AlertCircle },
  }
  const v = variants[status] || variants.pending
  const Icon = v.icon
  return (
    <Badge className={v.class}>
      <Icon className="mr-1 h-3 w-3" />
      {status}
    </Badge>
  )
}

function CacheBadge({ hit }: { hit: boolean }) {
  if (hit) {
    return <Badge className="bg-purple-100 text-purple-800 hover:bg-purple-100">Cache HIT</Badge>
  }
  return <Badge variant="outline">Cache miss</Badge>
}

function formatDate(iso: string | null): string {
  if (!iso) return '—'
  const d = new Date(iso)
  return d.toLocaleString()
}

function formatCost(cost: string): string {
  const n = parseFloat(cost)
  if (n === 0) return '$0.0000'
  return `$${n.toFixed(6)}`
}

export default function RequestsPage() {
  const { data, isLoading, error } = useRequests()

  if (isLoading) {
    return (
      <div className="space-y-4 p-6">
        <Skeleton className="h-8 w-48" />
        {[...Array(5)].map((_, i) => (
          <Skeleton key={i} className="h-24 w-full" />
        ))}
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center p-6">
        <div className="text-center">
          <AlertCircle className="mx-auto h-12 w-12 text-destructive" />
          <h3 className="mt-4 text-lg font-semibold">Failed to load requests</h3>
          <p className="text-muted-foreground">{error.message}</p>
        </div>
      </div>
    )
  }

  const requests = data?.data || []

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Request Logs</h1>
        <p className="text-muted-foreground">
          {data?.total || 0} total requests · showing latest {requests.length}
        </p>
      </div>

      <div className="h-[calc(100vh-200px)] overflow-auto space-y-3">
        {requests.length === 0 && (
          <Card>
            <CardContent className="py-12 text-center">
              <Database className="mx-auto h-12 w-12 text-muted-foreground" />
              <h3 className="mt-4 text-lg font-semibold">No requests yet</h3>
              <p className="text-muted-foreground">
                Make your first API call to see logs here.
              </p>
            </CardContent>
          </Card>
        )}

        {requests.map((req) => (
          <Card key={req.id} className="overflow-hidden">
            <CardHeader className="pb-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <StatusBadge status={req.status} />
                  <CacheBadge hit={req.cache_hit} />
                  {req.status_code && (
                    <Badge variant="outline">HTTP {req.status_code}</Badge>
                  )}
                </div>
                <span className="text-xs text-muted-foreground">
                  {formatDate(req.gateway_received_at)}
                </span>
              </div>
            </CardHeader>
            <CardContent className="pb-4 pt-0">
              <div className="grid grid-cols-2 gap-4 text-sm md:grid-cols-4">
                <div>
                  <p className="text-xs text-muted-foreground">Model</p>
                  <p className="font-medium">{req.model_routed || req.model_requested || '—'}</p>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">Tokens</p>
                  <p className="font-medium">
                    {req.prompt_tokens} prompt + {req.completion_tokens} completion
                    <span className="text-muted-foreground"> ({req.total_tokens} total)</span>
                  </p>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">Cost</p>
                  <p className="font-medium">{formatCost(req.total_cost)}</p>
                </div>
                <div>
                  <p className="text-xs text-muted-foreground">Latency</p>
                  <p className="font-medium">
                    {req.latency_total_ms !== null ? `${req.latency_total_ms}ms` : '—'}
                  </p>
                </div>
              </div>
              {req.error_message && (
                <div className="mt-3 rounded bg-destructive/10 px-3 py-2 text-xs text-destructive">
                  {req.error_message}
                </div>
              )}
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  )
}
