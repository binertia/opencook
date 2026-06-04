import { useState } from 'react'
import { useAuth } from '@/hooks/useAuth'
import { useUsers, useUpdateUserRole, useRemoveUser } from '@/hooks/useUsers'
import { InviteUserModal } from '@/components/users/InviteUserModal'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Badge } from '@/components/ui/badge'

const STATUS_OPTIONS = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'pending', label: 'Pending' },
  { value: 'suspended', label: 'Suspended' },
]

const ROLES = ['owner', 'admin', 'member', 'viewer'] as const

function getRoleBadgeVariant(role: string) {
  switch (role) {
    case 'owner':
      return 'default'
    case 'admin':
      return 'secondary'
    case 'member':
      return 'outline'
    case 'viewer':
      return 'outline'
    default:
      return 'outline'
  }
}

function formatDate(date: string | null) {
  if (!date) return 'Never'
  return new Date(date).toLocaleDateString()
}

export default function UsersPage() {
  const { user: currentUser } = useAuth()
  const orgId = currentUser?.organizations?.[0]?.org_id
  const currentRole = currentUser?.organizations?.[0]?.role || 'viewer'
  const isOwner = currentRole === 'owner'
  const isAdmin = currentRole === 'admin' || isOwner

  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState('all')
  const [removeUserId, setRemoveUserId] = useState<string | null>(null)

  const { data, isLoading, error } = useUsers(orgId, search, statusFilter)
  const updateRole = useUpdateUserRole()
  const removeUser = useRemoveUser()

  const users = data?.data || []
  const userToRemove = users.find((u) => u.id === removeUserId)

  const canChangeRole = (targetRole: string) => {
    if (isOwner) return true
    if (isAdmin) return targetRole !== 'owner'
    return false
  }

  const canRemove = (targetUser: { id: string; role: string }) => {
    if (targetUser.id === currentUser?.id) return false
    if (isOwner) return true
    if (isAdmin) return targetUser.role !== 'owner'
    return false
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Users</h1>
          <p className="text-muted-foreground">Manage organization members.</p>
        </div>
        {isAdmin && <InviteUserModal orgId={orgId} />}
      </div>

      <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
        <div className="flex-1">
          <Label htmlFor="search" className="sr-only">
            Search
          </Label>
          <Input
            id="search"
            placeholder="Search by name or email..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <div className="w-full sm:w-48">
          <Label htmlFor="status" className="sr-only">
            Status
          </Label>
          <Select value={statusFilter} onValueChange={setStatusFilter}>
            <SelectTrigger id="status">
              <SelectValue placeholder="Filter by status" />
            </SelectTrigger>
            <SelectContent>
              {STATUS_OPTIONS.map((opt) => (
                <SelectItem key={opt.value} value={opt.value}>
                  {opt.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {isLoading && (
        <p className="text-muted-foreground">Loading users...</p>
      )}

      {error && (
        <div className="rounded-md border border-destructive/50 bg-destructive/10 p-4 text-destructive">
          Failed to load users: {error.message}
        </div>
      )}

      {!isLoading && !error && (
        <div className="rounded-md border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Email</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Last Login</TableHead>
                <TableHead className="w-24" />
              </TableRow>
            </TableHeader>
            <TableBody>
              {users.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={6} className="text-center text-muted-foreground">
                    No users found.
                  </TableCell>
                </TableRow>
              ) : (
                users.map((u) => (
                  <TableRow key={u.id}>
                    <TableCell className="font-medium">{u.name}</TableCell>
                    <TableCell>{u.email}</TableCell>
                    <TableCell>
                      {canChangeRole(u.role) && u.id !== currentUser?.id ? (
                        <Select
                          value={u.role}
                          onValueChange={(role) =>
                            updateRole.mutate({ userId: u.id, role })
                          }
                          disabled={updateRole.isPending}
                        >
                          <SelectTrigger className="w-32">
                            <SelectValue />
                          </SelectTrigger>
                          <SelectContent>
                            {ROLES.filter((r) =>
                              isOwner ? true : r !== 'owner'
                            ).map((r) => (
                              <SelectItem key={r} value={r}>
                                {r.charAt(0).toUpperCase() + r.slice(1)}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                      ) : (
                        <Badge variant={getRoleBadgeVariant(u.role)}>
                          {u.role}
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell>
                      <Badge
                        variant={
                          u.status === 'active'
                            ? 'default'
                            : u.status === 'pending'
                              ? 'secondary'
                              : 'destructive'
                        }
                      >
                        {u.status}
                      </Badge>
                    </TableCell>
                    <TableCell>{formatDate(u.last_login_at)}</TableCell>
                    <TableCell>
                      {canRemove(u) && (
                        <Button
                          variant="ghost"
                          size="sm"
                          className="text-destructive hover:text-destructive"
                          onClick={() => setRemoveUserId(u.id)}
                        >
                          Remove
                        </Button>
                      )}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </div>
      )}

      <Dialog open={!!removeUserId} onOpenChange={() => setRemoveUserId(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Remove Member</DialogTitle>
            <DialogDescription>
              Are you sure you want to remove{' '}
              <strong>{userToRemove?.name}</strong>? This action cannot be
              undone.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setRemoveUserId(null)}>
              Cancel
            </Button>
            <Button
              variant="destructive"
              onClick={() => {
                if (removeUserId) {
                  removeUser.mutate(removeUserId, {
                    onSuccess: () => setRemoveUserId(null),
                  })
                }
              }}
              disabled={removeUser.isPending}
            >
              {removeUser.isPending ? 'Removing...' : 'Remove'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
