import { useState } from 'react'
import { X, CheckCircle, XCircle, Clock, RefreshCw, Eye, Filter, Search } from 'lucide-react'
import { useWebhookDeliveries, useRetryWebhookDelivery } from '@/hooks/useWebhooks'
import type { WebhookDelivery } from '@/hooks/useWebhooks'
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from '@/components/ui/tabs'
import { cn } from '@/lib/utils'

interface WebhookDeliveryLogProps {
  webhookId: string | null
  onClose: () => void
}

function StatusIcon({ status }: { status: string }) {
  if (status === 'delivered') {
    return <CheckCircle className="h-4 w-4 text-green-500" />
  }
  if (status === 'failed') {
    return <XCircle className="h-4 w-4 text-red-500" />
  }
  return <Clock className="h-4 w-4 text-yellow-500" />
}

function StatusBadge({ status }: { status: string }) {
  if (status === 'delivered') {
    return <Badge variant="default" className="bg-green-500">Delivered</Badge>
  }
  if (status === 'failed') {
    return <Badge variant="destructive">Failed</Badge>
  }
  return <Badge variant="secondary">Pending</Badge>
}

function JsonBlock({ value, label }: { value: unknown; label: string }) {
  const text = typeof value === 'string' ? value : JSON.stringify(value, null, 2)
  return (
    <div>
      <p className="text-xs text-muted-foreground mb-1">{label}</p>
      <pre className="text-xs bg-muted rounded-md p-3 overflow-auto max-h-60 font-mono">
        {text || '—'}
      </pre>
    </div>
  )
}

function DurationLabel({
  startedAt,
  completedAt,
}: {
  startedAt: string | null
  completedAt: string | null
}) {
  if (!startedAt || !completedAt) return <span>—</span>
  const ms = new Date(completedAt).getTime() - new Date(startedAt).getTime()
  if (ms < 1000) return <span>{ms}ms</span>
  return <span>{(ms / 1000).toFixed(2)}s</span>
}

function DeliveryDetail({
  delivery,
  onRetry,
  isRetrying,
  retryDeliveryId,
}: {
  delivery: WebhookDelivery
  onRetry: (id: string) => void
  isRetrying: boolean
  retryDeliveryId: string | undefined
}) {
  return (
    <div className="space-y-4">
      <Tabs defaultValue="overview">
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="request">Request</TabsTrigger>
          <TabsTrigger value="response">Response</TabsTrigger>
          <TabsTrigger value="payload">Payload</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-3">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <p className="text-xs text-muted-foreground">Event</p>
              <p className="font-medium text-sm">{delivery.event_type}</p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Status</p>
              <StatusBadge status={delivery.status} />
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Attempt</p>
              <p className="font-medium text-sm">#{delivery.attempt_number}</p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Response</p>
              <p className="font-medium text-sm">
                {delivery.response_status ? `HTTP ${delivery.response_status}` : '—'}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Scheduled</p>
              <p className="font-medium text-sm">
                {new Date(delivery.scheduled_at).toLocaleString()}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Started</p>
              <p className="font-medium text-sm">
                {delivery.started_at ? new Date(delivery.started_at).toLocaleString() : '—'}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Completed</p>
              <p className="font-medium text-sm">
                {delivery.completed_at ? new Date(delivery.completed_at).toLocaleString() : '—'}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">Duration</p>
              <p className="font-medium text-sm">
                <DurationLabel startedAt={delivery.started_at} completedAt={delivery.completed_at} />
              </p>
            </div>
          </div>

          {delivery.error_message && (
            <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3">
              <p className="text-xs font-medium text-destructive">Error</p>
              <p className="text-destructive text-sm">{delivery.error_message}</p>
            </div>
          )}

          <div className="flex justify-end">
            {delivery.status === 'failed' && (
              <Button
                onClick={() => onRetry(delivery.id)}
                disabled={isRetrying}
              >
                <RefreshCw className={cn('mr-2 h-4 w-4', isRetrying && retryDeliveryId === delivery.id && 'animate-spin')} />
                Retry Delivery
              </Button>
            )}
          </div>
        </TabsContent>

        <TabsContent value="request" className="space-y-3">
          <JsonBlock value={delivery.request_headers} label="Headers" />
          <JsonBlock value={delivery.request_body} label="Body" />
        </TabsContent>

        <TabsContent value="response" className="space-y-3">
          <div>
            <p className="text-xs text-muted-foreground mb-1">Status</p>
            <p className="font-medium text-sm">
              {delivery.response_status ? `HTTP ${delivery.response_status}` : '—'}
            </p>
          </div>
          <JsonBlock value={delivery.response_headers} label="Headers" />
          <JsonBlock value={delivery.response_body} label="Body" />
        </TabsContent>

        <TabsContent value="payload" className="space-y-3">
          <JsonBlock value={delivery.payload} label="Event Payload" />
        </TabsContent>
      </Tabs>
    </div>
  )
}

export function WebhookDeliveryLog({ webhookId, onClose }: WebhookDeliveryLogProps) {
  const { data, isLoading, error } = useWebhookDeliveries(webhookId)
  const retryDelivery = useRetryWebhookDelivery()

  const [filter, setFilter] = useState<string>('all')
  const [search, setSearch] = useState('')
  const [detailDelivery, setDetailDelivery] = useState<WebhookDelivery | null>(null)

  const deliveries = data?.data || []

  const filtered = deliveries.filter((d) => {
    if (filter === 'all') return true
    if (filter === 'success') return d.status === 'delivered'
    if (filter === 'failed') return d.status === 'failed'
    return true
  }).filter((d) => {
    if (!search.trim()) return true
    const term = search.toLowerCase()
    return d.event_type.toLowerCase().includes(term)
  })

  const handleRetry = (deliveryId: string) => {
    if (!webhookId) return
    retryDelivery.mutate({ webhookId, deliveryId })
  }

  return (
    <Dialog open={!!webhookId} onOpenChange={onClose}>
      <DialogContent className="max-w-3xl max-h-[80vh] overflow-auto">
        <DialogHeader>
          <div className="flex items-center justify-between">
            <DialogTitle>Delivery Log</DialogTitle>
            <Button variant="ghost" size="sm" onClick={onClose}>
              <X className="h-4 w-4" />
            </Button>
          </div>
        </DialogHeader>

        <div className="flex flex-col sm:flex-row items-start sm:items-center gap-2">
          <div className="relative flex-1 w-full">
            <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search by event type..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="pl-9 w-full"
            />
          </div>
          <div className="flex items-center gap-2">
            <Filter className="h-4 w-4 text-muted-foreground" />
            <Select value={filter} onValueChange={setFilter}>
              <SelectTrigger className="w-40">
                <SelectValue placeholder="Filter" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="all">All</SelectItem>
                <SelectItem value="success">Successful</SelectItem>
                <SelectItem value="failed">Failed</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="text-xs text-muted-foreground">
          {filtered.length} of {deliveries.length} deliveries
        </div>

        {error && (
          <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
            Failed to load deliveries: {error.message}
          </div>
        )}

        {isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-full" />
          </div>
        ) : filtered.length === 0 ? (
          <p className="text-sm text-muted-foreground py-4">
            {search.trim() || filter !== 'all'
              ? 'No deliveries match your filters.'
              : 'No delivery attempts yet.'}
          </p>
        ) : (
          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-8" />
                  <TableHead>Event</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Response</TableHead>
                  <TableHead>Time</TableHead>
                  <TableHead className="w-24" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {filtered.map((d) => (
                  <TableRow key={d.id}>
                    <TableCell>
                      <StatusIcon status={d.status} />
                    </TableCell>
                    <TableCell className="text-sm">{d.event_type}</TableCell>
                    <TableCell>
                      <StatusBadge status={d.status} />
                    </TableCell>
                    <TableCell className="text-sm">
                      {d.response_status ? (
                        <span
                          className={
                            d.response_status >= 200 && d.response_status < 300
                              ? 'text-green-600'
                              : 'text-red-600'
                          }
                        >
                          HTTP {d.response_status}
                        </span>
                      ) : d.error_message ? (
                        <span className="text-red-600 text-xs" title={d.error_message}>
                          {d.error_message.length > 30
                            ? d.error_message.slice(0, 30) + '...'
                            : d.error_message}
                        </span>
                      ) : (
                        <span className="text-muted-foreground text-xs">—</span>
                      )}
                    </TableCell>
                    <TableCell className="text-xs text-muted-foreground">
                      {d.completed_at
                        ? new Date(d.completed_at).toLocaleString()
                        : new Date(d.scheduled_at).toLocaleString()}
                    </TableCell>
                    <TableCell>
                      <div className="flex items-center gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => setDetailDelivery(d)}
                        >
                          <Eye className="h-4 w-4" />
                        </Button>
                        {d.status === 'failed' && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleRetry(d.id)}
                            disabled={retryDelivery.isPending}
                          >
                            <RefreshCw
                              className={cn(
                                'h-4 w-4',
                                retryDelivery.isPending && retryDelivery.variables?.deliveryId === d.id && 'animate-spin'
                              )}
                            />
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>
        )}

        {/* Detail Modal */}
        <Dialog open={!!detailDelivery} onOpenChange={() => setDetailDelivery(null)}>
          <DialogContent className="max-w-2xl max-h-[85vh] overflow-auto">
            <DialogHeader>
              <DialogTitle>Delivery Detail</DialogTitle>
            </DialogHeader>
            {detailDelivery && (
              <DeliveryDetail
                delivery={detailDelivery}
                onRetry={(id) => {
                  handleRetry(id)
                  setDetailDelivery(null)
                }}
                isRetrying={retryDelivery.isPending}
                retryDeliveryId={retryDelivery.variables?.deliveryId}
              />
            )}
          </DialogContent>
        </Dialog>
      </DialogContent>
    </Dialog>
  )
}
