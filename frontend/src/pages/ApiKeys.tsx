import { useState } from 'react'
import { Copy, KeyRound, Trash2 } from 'lucide-react'
import { useApiKeys, useCreateApiKey, useUpdateApiKey, useDeleteApiKey } from '@/hooks/useApiKeys'
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

function formatDate(date: string | null) {
  if (!date) return 'Never'
  return new Date(date).toLocaleDateString()
}

export default function ApiKeys() {
  const { data, isLoading, error } = useApiKeys()
  const createKey = useCreateApiKey()
  const updateKey = useUpdateApiKey()
  const deleteKey = useDeleteApiKey()

  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [newKeyName, setNewKeyName] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [deleteKeyId, setDeleteKeyId] = useState<string | null>(null)

  const keys = data?.data || []

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

      <Card>
        <CardHeader>
          <CardTitle>Active Keys</CardTitle>
          <CardDescription>
            {keys.length === 0 ? 'No API keys found.' : `${keys.length} key(s) configured.`}
          </CardDescription>
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <div className="space-y-2">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-8 w-full" />
            </div>
          ) : keys.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Create an API key to start making requests.
            </p>
          ) : (
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Name</TableHead>
                    <TableHead>Prefix</TableHead>
                    <TableHead>Status</TableHead>
                    <TableHead>Created</TableHead>
                    <TableHead className="w-24" />
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {keys.map((key) => (
                    <TableRow key={key.id}>
                      <TableCell className="font-medium">{key.name}</TableCell>
                      <TableCell className="font-mono text-xs">{key.prefix}</TableCell>
                      <TableCell>
                        <Badge
                          variant={key.status === 'active' ? 'default' : 'secondary'}
                        >
                          {key.status}
                        </Badge>
                      </TableCell>
                      <TableCell>{formatDate(key.created_at)}</TableCell>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          {key.status === 'active' && (
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
                  ))}
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
