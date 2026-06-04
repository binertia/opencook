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

export interface ProviderModel {
  id: string
  name: string
  context_window?: number
  capabilities?: string[]
  pricing?: {
    input_per_1m_tokens: number
    output_per_1m_tokens: number
    currency: string
  }
  status: 'active' | 'inactive'
}

export interface ProviderHealth {
  provider_id: string
  status: 'healthy' | 'degraded' | 'unhealthy' | 'unknown'
  latency_ms: number
  error_rate: number
  last_checked: string
  message?: string
}

export interface ProviderDetail extends Provider {
  models: ProviderModel[]
  health?: ProviderHealth
  routing_weight: number
  priority: number
}

export interface ProvidersListResponse {
  object: 'list'
  data: Provider[]
}

export interface HealthHistoryEntry {
  checked_at: string
  status: 'healthy' | 'degraded' | 'unhealthy'
  latency_ms: number
  error: string | null
}

export interface HealthHistoryResponse {
  object: 'list'
  data: HealthHistoryEntry[]
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

export function useProvider(providerId: string) {
  return useQuery<ProviderDetail, ApiError>({
    queryKey: ['provider', providerId],
    queryFn: async () => {
      const response = await api.get(`v1/providers/${providerId}`)
      return response.json<ProviderDetail>()
    },
    enabled: !!providerId,
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

export function useProviderHealthHistory(providerId: string, hours = 24) {
  return useQuery<HealthHistoryResponse, ApiError>({
    queryKey: ['provider-health-history', providerId, hours],
    queryFn: async () => {
      const response = await api.get(`v1/providers/${providerId}/health-history`, {
        searchParams: { hours: String(hours) },
      })
      return response.json<HealthHistoryResponse>()
    },
    enabled: !!providerId,
  })
}

export function useUpdateProviderModels(providerId: string) {
  const queryClient = useQueryClient()

  return useMutation<ProviderDetail, ApiError, ProviderModel[]>({
    mutationFn: async (models) => {
      const response = await api.put(`v1/providers/${providerId}`, {
        json: { models: models.map((m) => ({ ...m, enabled: m.status === 'active' })) },
      })
      return response.json<ProviderDetail>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['provider', providerId] })
      queryClient.invalidateQueries({ queryKey: ['providers'] })
    },
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
