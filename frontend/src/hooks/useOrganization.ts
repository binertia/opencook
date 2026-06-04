import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface Organization {
  id: string
  name: string
  display_name: string
  description: string
  metadata: Record<string, unknown>
  settings: {
    default_routing_strategy: 'cost' | 'latency' | 'quality' | 'fallback'
    allowed_providers: string[]
    blocked_models: string[]
    token_budget: {
      monthly_limit: number | null
      cost_budget_usd: number | null
      alert_threshold_percent: number
    }
  }
  created_at: string
  updated_at: string
  created_by: string
  status: string
}

export interface OrganizationUpdate {
  name?: string
  display_name?: string
  description?: string
  settings?: {
    default_routing_strategy?: 'cost' | 'latency' | 'quality' | 'fallback'
    allowed_providers?: string[]
    blocked_models?: string[]
    token_budget?: {
      monthly_limit?: number | null
      cost_budget_usd?: number | null
      alert_threshold_percent?: number
    }
  }
}

const ORG_QUERY_KEY = ['organization']

export function useOrganization(orgId?: string) {
  return useQuery<Organization, ApiError>({
    queryKey: ORG_QUERY_KEY,
    queryFn: async () => {
      const url = orgId ? `v1/organizations/${orgId}` : 'v1/organizations'
      const response = await api.get(url)
      const data = await response.json<Organization | { data: Organization[] }>()
      // If listing, take first org; if single, use directly
      if ('data' in data && Array.isArray(data.data)) {
        if (data.data.length === 0) throw new Error('No organization found')
        return data.data[0]
      }
      return data as Organization
    },
    enabled: true,
  })
}

export function useUpdateOrganization(orgId?: string) {
  const queryClient = useQueryClient()

  return useMutation<Organization, ApiError, OrganizationUpdate>({
    mutationFn: async (data) => {
      const url = orgId ? `v1/organizations/${orgId}` : 'v1/organizations'
      const response = await api.put(url, { json: data })
      return response.json<Organization>()
    },
    onSuccess: (updatedOrg) => {
      queryClient.setQueryData(ORG_QUERY_KEY, updatedOrg)
    },
  })
}

export { parseApiError }
