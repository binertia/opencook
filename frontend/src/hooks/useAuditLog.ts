import { useQuery } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface AuditEntry {
  id: string
  org_id: string
  user_id: string | null
  api_key_id: string | null
  action: string
  entity_type: string
  entity_id: string | null
  old_values: Record<string, unknown> | null
  new_values: Record<string, unknown> | null
  summary: string
  ip_address: string | null
  user_agent: string | null
  request_id: string | null
  created_at: string
}

export interface AuditListResponse {
  object: string
  data: AuditEntry[]
  total: number
  limit: number
  offset: number
}

export interface AuditLogFilters {
  action?: string
  entity_type?: string
  entity_id?: string
  user_id?: string
  start?: string
  end?: string
}

export function useAuditLog(
  orgId: string,
  filters: AuditLogFilters = {},
  limit = 50,
  offset = 0
) {
  const params = new URLSearchParams()
  params.set('limit', String(limit))
  params.set('offset', String(offset))
  if (filters.action) params.set('action', filters.action)
  if (filters.entity_type) params.set('entity_type', filters.entity_type)
  if (filters.entity_id) params.set('entity_id', filters.entity_id)
  if (filters.user_id) params.set('user_id', filters.user_id)
  if (filters.start) params.set('start', filters.start)
  if (filters.end) params.set('end', filters.end)

  return useQuery<AuditListResponse, ApiError>({
    queryKey: ['audit-log', orgId, filters, limit, offset],
    queryFn: async () => {
      const response = await api.get(
        `v1/organizations/${orgId}/audit-log?${params.toString()}`
      )
      return response.json<AuditListResponse>()
    },
    staleTime: 30_000,
  })
}

export { parseApiError }
