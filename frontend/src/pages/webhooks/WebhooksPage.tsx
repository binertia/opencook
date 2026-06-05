import { useState } from 'react'
import { WebhookIcon, Trash2, Activity, Clock, AlertTriangle } from 'lucide-react'
import {
  useWebhooks,
  useUpdateWebhook,
  useDeleteWebhook,
} from '@/hooks/useWebhooks'
import { CreateWebhookModal } from '@/components/webhooks/CreateWebhookModal'
import { WebhookDeliveryLog } from '@/components/webhooks/WebhookDeliveryLog'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import { Switch } from '@/components/ui/switch'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

const EVENT_LABELS: Record<string, string> = {
  'request.completed': 'Request Completed',
  'request.failed': 'Request Failed',
  'quota.warning': 'Quota Warning',
  'quota.exceeded': 'Quota Exceeded',
  'provider.error': 'Provider Error',
  'provider.recovered': 'Provider Recovered',
}

function StatusBadge({ status, consecutiveFailures }: { status: string; consecutiveFailures: number }) {
  if (status === 'failing' || consecutiveFailures > 0) {
    return (
      <Badge variant="destructive" className="gap-1">
        <AlertTriangle className="h-3 w-3" />
        Failing
      </Badge>
    )
  }
  if (status === 'inactive') {
    return <Badge variant="secondary">Inactive</Badge>
  }
  return (
    <Badge variant="default" className="gap-1">
      <Activity className="h-3 w-3" />
      Active
    </Badge>
  )
}

export default function WebhooksPage() {
  const { data, isLoading, error } = useWebhooks()
  const updateWebhook = useUpdateWebhook()
  const deleteWebhook = useDeleteWebhook()

  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [deleteWebhookId, setDeleteWebhookId] = useState<string | null>(null)
  const [selectedWebhookId, setSelectedWebhookId] = useState<string | null>(null)

  const webhooks = data?.data || []

  const handleToggleStatus = (webhook: { id: string; status: string }) => {
    const newStatus = webhook.status === 'active' ? 'inactive' : 'active'
    updateWebhook.mutate({ webhookId: webhook.id, status: newStatus })
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Webhooks</h1>
          <p className="text-muted-foreground">
            Manage webhook subscriptions and monitor delivery status.
          </p>
        </div>
        <Button onClick={() => setIsCreateOpen(true)}>
          <WebhookIcon className="mr-2 h-4 w-4" />
          Create Webhook
        </Button>
      </div>

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          Failed to load webhooks: {error.message}
        </div>
      )}

      <Card>
        <CardHeader>
          <CardTitle>Active Webhooks</CardTitle>
          <CardDescription>
            {webhooks.length === 0
              ? 'No webhooks configured.'
              : `${webhooks.length} webhook(s) configured.`}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : webhooks.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Create a webhook to receive event notifications.
            </p>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>URL</TableHead>
                    <TableHead>Events</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Last Delivery</TableHead>
                    <TableHead className="w-32" />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {webhooks.map((wh) => (
                    <TableRow key={wh.id}>
                      <TableCell className="font-medium">{wh.name}</TableCell>
                      <TableCell className="max-w-[200px] truncate text-xs">
                        {wh.url}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {wh.events.slice(0, 2).map((e) => (
                            <Badge key={e} variant="outline" className="text-xs">
                              {EVENT_LABELS[e] || e}
                            </Badge>
                          ))}
                          {wh.events.length > 2 && (
                            <Badge variant="outline" className="text-xs">
                              +{wh.events.length - 2}
                            </Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        <StatusBadge
                          status={wh.status}
                          consecutiveFailures={wh.consecutive_failures}
                        />
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-1 text-xs text-muted-foreground">
                          <Clock className="h-3 w-3" />
                          {wh.last_delivered_at
                            ? new Date(wh.last_delivered_at).toLocaleDateString()
                            : 'Never'}
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          <Switch
                            checked={wh.status === 'active'}
                            onCheckedChange={() => handleToggleStatus(wh)}
                          />
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setSelectedWebhookId(wh.id)}
                          >
                            Log
                          </Button>
                          <Button
                            variant="ghost"
                            size="sm"
                            className="text-destructive hover:text-destructive"
                            onClick={() => setDeleteWebhookId(wh.id)}
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Create Modal */}
      <CreateWebhookModal open={isCreateOpen} onOpenChange={setIsCreateOpen} />

      {/* Delivery Log */}
      <WebhookDeliveryLog
        webhookId={selectedWebhookId}
        onClose={() => setSelectedWebhookId(null)}
      />

      {/* Delete Confirmation */}
      <Dialog open={!!deleteWebhookId} onOpenChange={() => setDeleteWebhookId(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete Webhook</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete this webhook? This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setDeleteWebhookId(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (deleteWebhookId) {
                  deleteWebhook.mutate(deleteWebhookId, {
                    onSuccess: () => setDeleteWebhookId(null),
                  })
                }
              }}
              disabled={deleteWebhook.isPending}
            >
              {deleteWebhook.isPending ? 'Deleting...' : 'Delete'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
