import { useState } from 'react'
import { useParams, useNavigate, Link } from 'react-router-dom'
import {
  ArrowLeft,
  Edit,
  Trash2,
  AlertTriangle,
  Loader2,
  RefreshCw,
} from 'lucide-react'
import {
  useProvider,
  useProviderHealth,
  useProviderHealthHistory,
  useUpdateProviderModels,
  useTriggerHealthCheck,
} from '@/hooks/useProviders'
import { useDeleteProvider } from '@/hooks/useProviderMutations'
import { useProviderRoutingRules } from '@/hooks/useRoutingRules'
import { HealthChart } from '@/components/providers/HealthChart'
import { ModelList } from '@/components/providers/ModelList'
import { HealthIndicator } from '@/components/providers/HealthIndicator'
import { EditProviderModal } from '@/components/providers/EditProviderModal'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
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
import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'


export default function ProviderDetailPage() {
  const { providerId } = useParams<{ providerId: string }>()
  const navigate = useNavigate()
  const [showEdit, setShowEdit] = useState(false)
  const [showDelete, setShowDelete] = useState(false)

  const {
    data: provider,
    isLoading: providerLoading,
    error: providerError,
    refetch: refetchProvider,
  } = useProvider(providerId || '')

  const { data: health } = useProviderHealth(providerId || '')
  const { data: history, isLoading: historyLoading } = useProviderHealthHistory(
    providerId || '',
    24
  )

  const { rules } = useProviderRoutingRules(providerId || '')
  const updateModels = useUpdateProviderModels(providerId || '')
  const triggerCheck = useTriggerHealthCheck()
  const deleteProvider = useDeleteProvider()

  const handleToggleModel = (modelId: string, enabled: boolean) => {
    if (!provider) return
    const updated = provider.models.map((m) =>
      m.id === modelId ? { ...m, status: (enabled ? 'active' : 'inactive') as 'active' | 'inactive' } : m
    )
    updateModels.mutate(updated)
  }

  const handleDelete = async () => {
    if (!providerId) return
    await deleteProvider.mutateAsync(providerId)
    setShowDelete(false)
    navigate('/providers')
  }

  const recentErrors =
    history?.data
      .filter((entry) => entry.error !== null)
      .slice(0, 10) || []

  if (providerLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-48" />
        <Skeleton className="h-40 w-full" />
        <Skeleton className="h-64 w-full" />
      </div>
    )
  }

  if (providerError || !provider) {
    return (
      <div className="space-y-4">
        <Button variant="ghost" size="sm" asChild>
          <Link to="/providers">
            <ArrowLeft className="mr-2 h-4 w-4" />
            Back to Providers
          </Link>
        </Button>
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          <p>Failed to load provider: {providerError?.message || 'Not found'}</p>
          <Button variant="outline" size="sm" className="mt-2" onClick={() => refetchProvider()}>
            Retry
          </Button>
        </div>
      </div>
    )
  }

  const isReferenced = rules.length > 0

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="sm" asChild>
            <Link to="/providers">
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Link>
          </Button>
          <div>
            <h1 className="text-3xl font-bold tracking-tight">{provider.name}</h1>
            <p className="text-muted-foreground">
              Provider configuration and health monitoring.
            </p>
          </div>
        </div>
        <div className="flex gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => triggerCheck.mutate(provider.id)}
            disabled={triggerCheck.isPending}
          >
            <RefreshCw className={cn('mr-2 h-4 w-4', triggerCheck.isPending && 'animate-spin')} />
            Check Health
          </Button>
          <Button variant="outline" size="sm" onClick={() => setShowEdit(true)}>
            <Edit className="mr-2 h-4 w-4" />
            Edit
          </Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={() => setShowDelete(true)}
          >
            <Trash2 className="mr-2 h-4 w-4" />
            Delete
          </Button>
        </div>
      </div>

      {/* Provider Info Card */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Provider Info</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <div>
              <p className="text-sm text-muted-foreground">Kind</p>
              <Badge variant="outline" className="mt-1 capitalize">
                {provider.kind}
              </Badge>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Base URL</p>
              <p className="mt-1 text-sm font-medium break-all">{provider.base_url || '—'}</p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Status</p>
              <Badge
                variant={provider.status === 'active' ? 'default' : 'secondary'}
                className="mt-1"
              >
                {provider.status}
              </Badge>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Health</p>
              <div className="mt-1">
                <HealthIndicator health={health} size="sm" />
              </div>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Weight</p>
              <p className="mt-1 text-sm font-medium">{provider.routing_weight}</p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Priority</p>
              <p className="mt-1 text-sm font-medium">{provider.priority}</p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Created</p>
              <p className="mt-1 text-sm font-medium">
                {new Date(provider.created_at).toLocaleDateString()}
              </p>
            </div>
            <div>
              <p className="text-sm text-muted-foreground">Updated</p>
              <p className="mt-1 text-sm font-medium">
                {new Date(provider.updated_at).toLocaleDateString()}
              </p>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Models */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Models</CardTitle>
        </CardHeader>
        <CardContent>
          <ModelList
            models={provider.models || []}
            onToggle={handleToggleModel}
            isUpdating={updateModels.isPending}
          />
        </CardContent>
      </Card>

      {/* Health History Chart */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Health History (Last 24h)</CardTitle>
        </CardHeader>
        <CardContent>
          {historyLoading ? (
            <Skeleton className="h-72 w-full" />
          ) : (
            <HealthChart data={history?.data || []} />
          )}
        </CardContent>
      </Card>

      {/* Recent Errors */}
      <Card>
        <CardHeader>
          <CardTitle className="text-lg">Recent Errors</CardTitle>
        </CardHeader>
        <CardContent>
          {recentErrors.length === 0 ? (
            <p className="text-sm text-muted-foreground">No errors in the last 24 hours.</p>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Timestamp</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Error</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {recentErrors.map((entry, idx) => (
                    <TableRow key={idx}>
                      <TableCell className="text-sm">
                        {new Date(entry.checked_at).toLocaleString()}
                      </TableCell>
                      <TableCell>
                        <Badge
                          variant={
                            entry.status === 'healthy'
                              ? 'default'
                              : entry.status === 'degraded'
                                ? 'secondary'
                                : 'destructive'
                          }
                        >
                          {entry.status}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-sm text-destructive">
                        {entry.error}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Edit Modal */}
      <EditProviderModal
        provider={provider}
        open={showEdit}
        onOpenChange={setShowEdit}
      />

      {/* Delete Confirmation */}
      <Dialog open={showDelete} onOpenChange={setShowDelete}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              {isReferenced ? (
                <>
                  <AlertTriangle className="h-5 w-5 text-yellow-500" />
                  Cannot Delete Provider
                </>
              ) : (
                <>
                  <Trash2 className="h-5 w-5 text-destructive" />
                  Delete Provider
                </>
              )}
            </DialogTitle>
            <DialogDescription asChild>
              {isReferenced ? (
                <div className="space-y-2 text-sm text-muted-foreground">
                  <p>
                    This provider is referenced by {rules.length} active routing
                    {rules.length === 1 ? ' rule' : ' rules'}. You must remove it from
                    the following rules before deleting:
                  </p>
                  <ul className="list-disc pl-5">
                    {rules.map((rule) => (
                      <li key={rule.id}>{rule.name}</li>
                    ))}
                  </ul>
                </div>
              ) : (
                <span>
                  Are you sure you want to delete <strong>{provider.name}</strong>? This
                  action cannot be undone.
                </span>
              )}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setShowDelete(false)}>
              {isReferenced ? 'Close' : 'Cancel'}
            </Button>
            {!isReferenced && (
              <Button
                variant="destructive"
                onClick={handleDelete}
                disabled={deleteProvider.isPending}
              >
                {deleteProvider.isPending && (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                )}
                Delete
              </Button>
            )}
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
