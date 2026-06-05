import { useState, useMemo } from 'react'
import { KeyRound, Trash2, Edit2, Search } from 'lucide-react'
import {
  useApiKeys,
  useCreateApiKey,
  useUpdateApiKey,
  useDeleteApiKey,
  useKeyUsage,
} from '@/hooks/useApiKeys'
import { CreateKeyModal } from '@/components/keys/CreateKeyModal'
import { KeyDisplayModal } from '@/components/keys/KeyDisplayModal'
import { RevokeKeyDialog } from '@/components/keys/RevokeKeyDialog'
import { EditKeyModal } from '@/components/keys/EditKeyModal'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
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
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { Skeleton } from '@/components/ui/skeleton'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { ApiKey, CreateApiKeyRequest } from '@/hooks/useApiKeys'

function formatDate(date: string | null) {
  if (!date) return 'Never'
  return new Date(date).toLocaleDateString()
}

function isExpired(expiresAt: string | null): boolean {
  if (!expiresAt) return false
  return new Date(expiresAt) < new Date()
}

type StatusFilter = 'all' | 'active' | 'revoked' | 'expired'

export default function ApiKeys() {
  const { data, isLoading, error } = useApiKeys()
  const { data: usageData } = useKeyUsage('30d')
  const createKey = useCreateApiKey()
  const updateKey = useUpdateApiKey()
  const deleteKey = useDeleteApiKey()

  const [showCreate, setShowCreate] = useState(false)
  const [displayedKey, setDisplayedKey] = useState<string | null>(null)
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null)
  const [editKey, setEditKey] = useState<ApiKey | null>(null)
  const [revokeKey, setRevokeKey] = useState<ApiKey | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all')

  const usageMap = useMemo(() => {
    const map = new Map<string, { requests: number; cost_usd: number }>()
    usageData?.data.forEach((item) => {
      map.set(item.api_key_id, { requests: item.requests, cost_usd: item.cost_usd })
    })
    return map
  }, [usageData])

  const keys = data?.data || []

  const filteredKeys = useMemo(() => {
    return keys.filter((key) => {
      const matchesSearch =
        searchQuery === '' ||
        key.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
        key.prefix.toLowerCase().includes(searchQuery.toLowerCase())

      const expired = isExpired(key.expires_at)
      let matchesStatus = true
      if (statusFilter === 'active') {
        matchesStatus = key.status === 'active' && !expired
      } else if (statusFilter === 'revoked') {
        matchesStatus = key.status === 'revoked'
      } else if (statusFilter === 'expired') {
        matchesStatus = expired
      }

      return matchesSearch && matchesStatus
    })
  }, [keys, searchQuery, statusFilter])

  const handleCreate = (payload: CreateApiKeyRequest) => {
    createKey.mutate(payload, {
      onSuccess: (data) => {
        setShowCreate(false)
        setDisplayedKey(data.key)
      },
    })
  }

  const handleRevoke = () => {
    if (!revokeKey) return
    updateKey.mutate(
      { keyId: revokeKey.id, status: 'revoked' },
      {
        onSuccess: () => setRevokeKey(null),
      }
    )
  }

  const handleEdit = (keyId: string, data: { name?: string; scopes?: string[]; rate_limit_rps?: number; expires_at?: string }) => {
    updateKey.mutate(
      { keyId, ...data },
      {
        onSuccess: () => setEditKey(null),
      }
    )
  }

  const getStatusBadge = (key: (typeof keys)[0]) => {
    if (key.status === 'revoked') {
      return <Badge variant="destructive">Revoked</Badge>
    }
    if (isExpired(key.expires_at)) {
      return <Badge variant="secondary">Expired</Badge>
    }
    return <Badge variant="default">Active</Badge>
  }

  const isReadOnly = (key: ApiKey) => key.status === 'revoked' || isExpired(key.expires_at)

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">API Keys</h1>
          <p className="text-muted-foreground">
            Manage API keys for your organization.
          </p>
        </div>
        <Button onClick={() => setShowCreate(true)}>
          <KeyRound className="mr-2 h-4 w-4" />
          Create Key
        </Button>
      </div>

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          Failed to load API keys: {error.message}
        </div>
      )}

      {/* Filters */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            placeholder="Search by name or prefix..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-8"
          />
        </div>
        <Select value={statusFilter} onValueChange={(v) => setStatusFilter(v as StatusFilter)}>
          <SelectTrigger className="w-[160px]">
            <SelectValue placeholder="Filter by status" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Status</SelectItem>
            <SelectItem value="active">Active</SelectItem>
            <SelectItem value="revoked">Revoked</SelectItem>
            <SelectItem value="expired">Expired</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>API Keys</CardTitle>
          <CardDescription>
            {filteredKeys.length === 0
              ? 'No API keys found.'
              : `${filteredKeys.length} key(s) configured.`}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : filteredKeys.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              {keys.length === 0
                ? 'Create an API key to start making requests.'
                : 'No keys match your filters.'}
            </p>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Prefix</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Scopes</TableHead>
                    <TableHead>Last Used</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead>Usage (30d)</TableHead>
                    <TableHead className="w-32">Actions</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredKeys.map((key) => {
                    const usage = usageMap.get(key.id)
                    return (
                      <TableRow key={key.id}>
                        <TableCell className="font-medium">{key.name}</TableCell>
                        <TableCell className="font-mono text-xs" title={key.prefix}>
                          {key.prefix}...
                        </TableCell>
                        <TableCell>{getStatusBadge(key)}</TableCell>
                        <TableCell>
                          <div className="flex flex-wrap gap-1">
                            {(key.scopes || ['all']).map((scope) => (
                              <Badge key={scope} variant="outline" className="text-xs">
                                {scope}
                              </Badge>
                            ))}
                          </div>
                        </TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                          {key.last_used_at ? formatDate(key.last_used_at) : 'Never'}
                        </TableCell>
                        <TableCell className="text-sm text-muted-foreground">
                          {formatDate(key.created_at)}
                        </TableCell>
                        <TableCell>
                          {usage ? (
                            <div className="text-sm">
                              <div>{usage.requests.toLocaleString()} reqs</div>
                              <div className="text-muted-foreground">
                                ${usage.cost_usd.toFixed(4)}
                              </div>
                            </div>
                          ) : (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </TableCell>
                        <TableCell>
                          <div className="flex items-center gap-1">
                            {!isReadOnly(key) && (
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => setEditKey(key)}
                                title="Edit key"
                              >
                                <Edit2 className="h-4 w-4" />
                              </Button>
                            )}
                            {!isReadOnly(key) && (
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => setRevokeKey(key)}
                              >
                                Revoke
                              </Button>
                            )}
                            <Button
                              variant="ghost"
                              size="sm"
                              className="text-destructive hover:text-destructive"
                              onClick={() => setDeleteKeyId(key.id)}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </TableCell>
                      </TableRow>
                    )
                  })}
                </TableBody>
              </Table>
            </div>
          )}
        </CardContent>
      </Card>

      <CreateKeyModal
        open={showCreate}
        onOpenChange={setShowCreate}
        onSubmit={handleCreate}
        isPending={createKey.isPending}
      />

      <KeyDisplayModal
        apiKey={displayedKey}
        open={!!displayedKey}
        onOpenChange={() => setDisplayedKey(null)}
      />

      <EditKeyModal
        apiKey={editKey}
        open={!!editKey}
        onOpenChange={() => setEditKey(null)}
        onSubmit={handleEdit}
        isPending={updateKey.isPending}
      />

      <RevokeKeyDialog
        open={!!revokeKey}
        onOpenChange={() => setRevokeKey(null)}
        keyName={revokeKey?.name || ''}
        lastUsed={revokeKey?.last_used_at || null}
        onConfirm={handleRevoke}
        isPending={updateKey.isPending}
      />

      {/* Delete Confirmation Dialog */}
      <Dialog open={!!deleteKeyId} onOpenChange={() => setDeleteKeyId(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Delete API Key</DialogTitle>
            <DialogDescription>
              Are you sure you want to delete this API key? This action cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setDeleteKeyId(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (deleteKeyId) {
                  deleteKey.mutate(deleteKeyId, {
                    onSuccess: () => setDeleteKeyId(null),
                  })
                }
              }}
              disabled={deleteKey.isPending}
            >
              {deleteKey.isPending ? 'Deleting...' : 'Delete'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
