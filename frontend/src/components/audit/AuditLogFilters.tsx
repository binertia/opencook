import { Search, X } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { AuditLogFilters } from '@/hooks/useAuditLog'

const ACTION_OPTIONS = [
  { value: '', label: 'All Actions' },
  { value: 'create', label: 'Create' },
  { value: 'update', label: 'Update' },
  { value: 'delete', label: 'Delete' },
  { value: 'login', label: 'Login' },
  { value: 'logout', label: 'Logout' },
  { value: 'api_key.created', label: 'API Key Created' },
  { value: 'api_key.revoked', label: 'API Key Revoked' },
  { value: 'provider.created', label: 'Provider Created' },
  { value: 'provider.updated', label: 'Provider Updated' },
  { value: 'provider.deleted', label: 'Provider Deleted' },
  { value: 'quota.exceeded', label: 'Quota Exceeded' },
  { value: 'quota.warning', label: 'Quota Warning' },
  { value: 'webhook.created', label: 'Webhook Created' },
  { value: 'webhook.deleted', label: 'Webhook Deleted' },
  { value: 'routing_rule.created', label: 'Routing Rule Created' },
  { value: 'routing_rule.updated', label: 'Routing Rule Updated' },
  { value: 'routing_rule.deleted', label: 'Routing Rule Deleted' },
  { value: 'settings.updated', label: 'Settings Updated' },
  { value: 'billing.updated', label: 'Billing Updated' },
  { value: 'user.role_changed', label: 'User Role Changed' },
]

const ENTITY_OPTIONS = [
  { value: '', label: 'All Resources' },
  { value: 'api_key', label: 'API Key' },
  { value: 'provider', label: 'Provider' },
  { value: 'user', label: 'User' },
  { value: 'webhook', label: 'Webhook' },
  { value: 'routing_rule', label: 'Routing Rule' },
  { value: 'organization', label: 'Organization' },
]

interface AuditLogFiltersProps {
  filters: AuditLogFilters
  onChange: (filters: AuditLogFilters) => void
}

export function AuditLogFilters({ filters, onChange }: AuditLogFiltersProps) {
  const hasFilters =
    filters.action || filters.entity_type || filters.entity_id || filters.user_id

  const clearFilters = () => {
    onChange({})
  }

  return (
    <div className="flex flex-wrap items-center gap-3">
      <div className="w-48">
        <Select
          value={filters.action || ''}
          onValueChange={(v) => onChange({ ...filters, action: v || undefined })}
        >
          <SelectTrigger>
            <SelectValue placeholder="Action" />
          </SelectTrigger>
          <SelectContent>
            {ACTION_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="w-48">
        <Select
          value={filters.entity_type || ''}
          onValueChange={(v) =>
            onChange({ ...filters, entity_type: v || undefined })
          }
        >
          <SelectTrigger>
            <SelectValue placeholder="Resource Type" />
          </SelectTrigger>
          <SelectContent>
            {ENTITY_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <div className="relative w-56">
        <Search className="absolute left-2.5 top-2.5 h-4 w-4 text-muted-foreground" />
        <Input
          placeholder="Search by resource ID..."
          className="pl-8"
          value={filters.entity_id || ''}
          onChange={(e) =>
            onChange({
              ...filters,
              entity_id: e.target.value || undefined,
            })
          }
        />
      </div>

      {hasFilters && (
        <Button variant="ghost" size="sm" onClick={clearFilters}>
          <X className="mr-1 h-4 w-4" />
          Clear
        </Button>
      )}
    </div>
  )
}
