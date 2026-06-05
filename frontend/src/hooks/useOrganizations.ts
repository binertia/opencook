import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface UserOrg {
  org_id: string
  org_name: string
  slug: string
  role: string
}

export interface SwitchOrgRequest {
  org_id: string
}

export interface SwitchOrgResponse {
  access_token: string
  refresh_token: string
  token_type: string
  expires_in: number
  csrf_token: string
  user: {
    id: string
    email: string
    name: string
    role: string
    permissions: string[]
    organizations: UserOrg[]
  }
}

const ORGS_QUERY_KEY = ['organizations']

export function useOrganizations() {
  return useQuery<UserOrg[], ApiError>({
    queryKey: ORGS_QUERY_KEY,
    queryFn: async () => {
      const response = await api.get('v1/auth/me')
      const data = await response.json<{ organizations: UserOrg[] }>()
      return data.organizations || []
    },
    enabled: true,
  })
}

export function useSwitchOrg() {
  const queryClient = useQueryClient()

  return useMutation<SwitchOrgResponse, ApiError, SwitchOrgRequest>({
    mutationFn: async (data) => {
      const response = await api.post('v1/auth/switch-org', { json: data })
      return response.json<SwitchOrgResponse>()
    },
    onSuccess: () => {
      // Invalidate all queries to ensure fresh data for the new org context.
      queryClient.invalidateQueries({ queryKey: ['organization'] })
      queryClient.invalidateQueries({ queryKey: ['dashboard'] })
      queryClient.invalidateQueries({ queryKey: ['analytics'] })
      queryClient.invalidateQueries({ queryKey: ['api-keys'] })
      queryClient.invalidateQueries({ queryKey: ['providers'] })
      queryClient.invalidateQueries({ queryKey: ['users'] })
      queryClient.invalidateQueries({ queryKey: ['webhooks'] })
      queryClient.invalidateQueries({ queryKey: ['quotas'] })
      // Hard reload to ensure all state is fresh with the new org context.
      window.location.reload()
    },
  })
}

export function useCreateOrganization() {
  const queryClient = useQueryClient()

  return useMutation<
    { id: string; name: string; slug: string; status: string; plan_tier: string; created_at: string },
    ApiError,
    { name: string; billing_email?: string }
  >({
    mutationFn: async (data) => {
      const response = await api.post('v1/organizations', { json: data })
      return response.json<{
        id: string
        name: string
        slug: string
        status: string
        plan_tier: string
        created_at: string
      }>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ORGS_QUERY_KEY })
      queryClient.invalidateQueries({ queryKey: ['organization'] })
    },
  })
}

export { parseApiError }
