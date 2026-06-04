import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface Provider {
  id: string
  name: string
  kind: string
  base_url: string
  status: 'active' | 'inactive'
  created_at: string
  updated_at: string
}

export interface ProviderHealth {
  provider_id: string
  status: 'healthy' | 'degraded' | 'unhealthy' | 'unknown'
  latency_ms: number
  error_rate: number
  last_checked: string
  message?: string
}

export interface ProvidersListResponse {
  object: 'list'
  data: Provider[]
}

const PROVIDERS_QUERY_KEY = ['providers']

export function useProviders() {
  return useQuery<ProvidersListResponse, ApiError>({
    queryKey: PROVIDERS_QUERY_KEY,
    queryFn: async () => {
      const response = await api.get('v1/providers')
      return response.json<ProvidersListResponse>()
    },
  })
}

export function useProviderHealth(providerId: string) {
  return useQuery<ProviderHealth, ApiError>({
    queryKey: ['provider-health', providerId],
    queryFn: async () => {
      const response = await api.get(`v1/providers/${providerId}/health`)
      return response.json<ProviderHealth>()
    },
    refetchInterval: 30_000,
    enabled: !!providerId,
  })
}

export function useTriggerHealthCheck() {
  const queryClient = useQueryClient()

  return useMutation<ProviderHealth, ApiError, string>({
    mutationFn: async (providerId) => {
      const response = await api.post(`v1/providers/${providerId}/health`)
      return response.json<ProviderHealth>()
    },
    onSuccess: (_, providerId) => {
      queryClient.invalidateQueries({ queryKey: ['provider-health', providerId] })
    },
  })
}

export { parseApiError }
