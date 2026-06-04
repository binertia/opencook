import { useMutation, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface CreateProviderRequest {
  name: string
  kind: 'openai' | 'anthropic' | 'gemini' | 'ollama' | 'custom'
  api_key?: string
  base_url?: string
  models?: string[]
  health_check_interval_seconds?: number
  health_check_timeout_seconds?: number
  health_check_model?: string
  weight?: number
  priority?: number
}

export interface UpdateProviderRequest {
  name?: string
  api_key?: string
  base_url?: string
  models?: string[]
  health_check_interval_seconds?: number
  health_check_timeout_seconds?: number
  health_check_model?: string
  weight?: number
  priority?: number
  status?: 'active' | 'inactive'
}

export interface TestConnectionResponse {
  success: boolean
  latency_ms: number
  message?: string
}

export function useCreateProvider() {
  const queryClient = useQueryClient()

  return useMutation<unknown, ApiError, CreateProviderRequest>({
    mutationFn: async (data) => {
      const response = await api.post('v1/providers', { json: data })
      return response.json()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['providers'] })
    },
  })
}

export function useUpdateProvider(providerId: string) {
  const queryClient = useQueryClient()

  return useMutation<unknown, ApiError, UpdateProviderRequest>({
    mutationFn: async (data) => {
      const response = await api.put(`v1/providers/${providerId}`, { json: data })
      return response.json()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['providers'] })
      queryClient.invalidateQueries({ queryKey: ['provider-health', providerId] })
    },
  })
}

export function useDeleteProvider() {
  const queryClient = useQueryClient()

  return useMutation<void, ApiError, string>({
    mutationFn: async (providerId) => {
      await api.delete(`v1/providers/${providerId}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['providers'] })
    },
  })
}

export function useTestConnection() {
  return useMutation<TestConnectionResponse, ApiError, { providerId?: string; config: CreateProviderRequest }>({
    mutationFn: async ({ providerId, config }) => {
      const url = providerId
        ? `v1/providers/${providerId}/test`
        : 'v1/providers/test'
      const response = await api.post(url, { json: config })
      return response.json<TestConnectionResponse>()
    },
  })
}

export { parseApiError }
