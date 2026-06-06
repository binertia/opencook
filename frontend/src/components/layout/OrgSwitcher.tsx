import { useState } from 'react'
import { Building2, Check, ChevronDown, Plus } from 'lucide-react'
import { useOrganizations, useSwitchOrg } from '@/hooks/useOrganizations'
import { useAuthStore } from '@/store/authStore'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useCreateOrganization } from '@/hooks/useOrganizations'

export function OrgSwitcher() {
  const { data: orgs, isLoading } = useOrganizations()
  const switchOrg = useSwitchOrg()
  const createOrg = useCreateOrganization()
  const [isCreateOpen, setIsCreateOpen] = useState(false)
  const [newOrgName, setNewOrgName] = useState('')
  const [newOrgEmail, setNewOrgEmail] = useState('')

  // Derive active org from the authenticated user's organizations.
  const user = useAuthStore((s) => s.user)
  const activeOrgId =
    user?.organizations?.find((o) => o.org_id === user?.organizations?.[0]?.org_id)?.org_id ??
    orgs?.[0]?.org_id

  const activeOrg = orgs?.find((o) => o.org_id === activeOrgId) || orgs?.[0]

  const handleSwitch = (orgId: string) => {
    switchOrg.mutate({ org_id: orgId })
  }

  const handleCreate = (e: React.FormEvent) => {
    e.preventDefault()
    if (!newOrgName.trim()) return
    createOrg.mutate(
      { name: newOrgName.trim(), billing_email: newOrgEmail.trim() || undefined },
      {
        onSuccess: (data) => {
          setIsCreateOpen(false)
          setNewOrgName('')
          setNewOrgEmail('')
          // Switch to the newly created org.
          handleSwitch(data.id)
        },
      }
    )
  }

  if (isLoading || !orgs || orgs.length === 0) {
    return (
      <Button variant="ghost" size="sm" className="h-9 gap-1 px-2" disabled>
        <Building2 className="h-4 w-4" />
        <span className="hidden sm:inline">Loading…</span>
      </Button>
    )
  }

  return (
    <Dialog open={isCreateOpen} onOpenChange={setIsCreateOpen}>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="sm" className="h-9 gap-1 px-2">
            <Building2 className="h-4 w-4" />
            <span className="hidden max-w-[120px] truncate sm:inline">
              {activeOrg?.org_name || 'Select org'}
            </span>
            <ChevronDown className="h-3 w-3 text-muted-foreground" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-56" align="start">
          <DropdownMenuLabel>Organizations</DropdownMenuLabel>
          <DropdownMenuSeparator />
          {orgs.map((org) => (
            <DropdownMenuItem
              key={org.org_id}
              onClick={() => handleSwitch(org.org_id)}
              className="cursor-pointer"
            >
              <div className="flex flex-1 items-center justify-between">
                <span className="truncate">{org.org_name}</span>
                {org.org_id === activeOrg?.org_id && (
                  <Check className="ml-2 h-4 w-4 text-primary" />
                )}
              </div>
            </DropdownMenuItem>
          ))}
          <DropdownMenuSeparator />
          <DialogTrigger asChild>
            <DropdownMenuItem className="cursor-pointer">
              <Plus className="mr-2 h-4 w-4" />
              Create organization
            </DropdownMenuItem>
          </DialogTrigger>
        </DropdownMenuContent>
      </DropdownMenu>

      <DialogContent>
        <form onSubmit={handleCreate}>
          <DialogHeader>
            <DialogTitle>Create organization</DialogTitle>
            <DialogDescription>
              Create a new organization. You will be the owner automatically.
            </DialogDescription>
          </DialogHeader>
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="org-name">Organization name</Label>
              <Input
                id="org-name"
                value={newOrgName}
                onChange={(e) => setNewOrgName(e.target.value)}
                placeholder="Acme Corp"
                required
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="billing-email">Billing email (optional)</Label>
              <Input
                id="billing-email"
                type="email"
                value={newOrgEmail}
                onChange={(e) => setNewOrgEmail(e.target.value)}
                placeholder="billing@example.com"
              />
            </div>
          </div>
          <DialogFooter>
            <Button type="submit" disabled={createOrg.isPending}>
              {createOrg.isPending ? 'Creating…' : 'Create'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
