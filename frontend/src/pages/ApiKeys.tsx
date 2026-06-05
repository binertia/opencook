import { useState, useMemo } from 'react'
import { Copy, KeyRound, Trash2, Edit2, Search } from 'lucide-react'
import {
  useApiKeys,
  useCreateApiKey,
  useUpdateApiKey,
  useDeleteApiKey,
  useKeyUsage,
} from '@/hooks/useApiKeys'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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

function formatDate(date: string | null) {
  if (!date) return 'Never'
  return new Date(date).toLocaleDateString()
}

function formatRelativeTime(date: string | null) {
  if (!date) return 'Never'
  const d = new Date(date)
  const now = new Date()
  const diffMs = now.getTime() - d.getTime()
  const diffMins = Math.floor(diffMs / 60000)
  const diffHours = Math.floor(diffMins / 60)
  const diffDays = Math.floor(diffHours / 24)

  if (diffMins < 1) return 'Just now'
  if (diffMins < 60) return `${diffMins}m ago`
  if (diffHours < 24) return `${diffHours}h ago`
  if (diffDays < 30) return `${diffDays}d ago`
  return d.toLocaleDateString()
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

  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [newKeyName, setNewKeyName] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null)
  const [editKeyId, setEditKeyId] = useState<string | null>(null)
  const [editKeyName, setEditKeyName] = useState('')
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

  const handleCreate = () => {
    if (!newKeyName.trim()) return
    createKey.mutate(
      { name: newKeyName.trim() },
      {
        onSuccess: (data) => {
          setCreatedKey(data.key)
          setNewKeyName('')
        },
      }
    )
  }

  const handleCopy = (text: string) => {
    navigator.clipboard.writeText(text)
  }

  const handleEdit = (keyId: string, name: string) => {
    setEditKeyId(keyId)
    setEditKeyName(name)
  }

  const handleSaveEdit = () => {
    if (!editKeyId || !editKeyName.trim()) return
    updateKey.mutate(
      { keyId: editKeyId, name: editKeyName.trim() },
      {
        onSuccess: () => {
          setEditKeyId(null)
          setEditKeyName('')
        },
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

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">API Keys</h1>
          <p className="text-muted-foreground">
            Manage API keys for your organization.
          </p>
        </div>
        <Button onClick={() => { setIsCreateOpen(true); setCreatedKey(null) }}>
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
                          {formatRelativeTime(key.last_used_at)}
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
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => handleEdit(key.id, key.name)}
                              title="Edit name"
                            >
                              <Edit2 className="h-4 w-4" />
                            </Button>
                            {key.status === 'active' && !isExpired(key.expires_at) && (
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() =>
                                  updateKey.mutate({ keyId: key.id, status: 'revoked' })
                                }
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

      {/* Create Key Dialog */}
      <Dialog open={isCreateOpen} onOpenChange={setIsCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Create API Key</DialogTitle>
            <DialogDescription>
              {createdKey
                ? 'Copy your API key now. You will not be able to see it again.'
                : 'Give your new API key a name.'}
            </DialogDescription>
          </DialogHeader>

          {createdKey ? (
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <code className="flex-1 rounded bg-muted px-2 py-1 text-sm break-all">
                  {createdKey}
                </code>
                <Button variant="outline" size="sm" onClick={() => handleCopy(createdKey)}>
                  <Copy className="h-4 w-4" />
                </Button>
              </div>
              <DialogFooter>
                <Button onClick={() => { setIsCreateOpen(false); setCreatedKey(null) }}>
                  Done
                </Button>
              </DialogFooter>
            </div>
          ) : (
            <>
              <div className="space-y-2">
                <Label htmlFor="key-name">Key Name</Label>
                <Input
                  id="key-name"
                  placeholder="e.g. Production API Key"
                  value={newKeyName}
                  onChange={(e) => setNewKeyName(e.target.value)}
                />
              </div>
              <DialogFooter>
                <Button variant="ghost" onClick={() => setIsCreateOpen(false)}>
                  Cancel
                </Button>
                <Button onClick={handleCreate} disabled={createKey.isPending || !newKeyName.trim()}>
                  {createKey.isPending ? 'Creating...' : 'Create'}
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>

      {/* Edit Name Dialog */}
      <Dialog open={!!editKeyId} onOpenChange={() => setEditKeyId(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Edit API Key Name</DialogTitle>
            <DialogDescription>Update the display name for this API key.</DialogDescription>
          </DialogHeader>
          <div className="space-y-2">
            <Label htmlFor="edit-key-name">Key Name</Label>
            <Input
              id="edit-key-name"
              placeholder="Key name"
              value={editKeyName}
              onChange={(e) => setEditKeyName(e.target.value)}
            />
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setEditKeyId(null)}>
              Cancel
            </Button>
            <Button
              onClick={handleSaveEdit}
              disabled={updateKey.isPending || !editKeyName.trim()}
            >
              {updateKey.isPending ? 'Saving...' : 'Save'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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
