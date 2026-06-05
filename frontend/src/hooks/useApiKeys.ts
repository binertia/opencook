import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface ApiKey {
  id: string
  name: string
  prefix: string
  scopes: string[]
  rate_limit_rps: number
  status: 'active' | 'revoked'
  expires_at: string | null
  last_used_at: string | null
  created_at: string
}

export interface ApiKeysListResponse {
  object: 'list'
  data: ApiKey[]
}

export interface KeyUsageItem {
  api_key_id: string
  key_name: string
  key_prefix: string
  key_status: string
  requests: number
  tokens: number
  prompt_tokens: number
  completion_tokens: number
  cost_usd: number
  avg_latency_ms: number
}

export interface KeyUsageResponse {
  data: KeyUsageItem[]
}

export interface CreateApiKeyRequest {
  name: string
  scopes?: string[]
  rate_limit_rps?: number
  expires_at?: string
}

export interface CreateApiKeyResponse extends ApiKey {
  key: string // plain key — only returned on creation
}

const API_KEYS_QUERY_KEY = ['api-keys']

export function useApiKeys() {
  return useQuery<ApiKeysListResponse, ApiError>({
    queryKey: API_KEYS_QUERY_KEY,
    queryFn: async () => {
      const response = await api.get('v1/api-keys')
      return response.json<ApiKeysListResponse>()
    },
  })
}

export function useCreateApiKey() {
  const queryClient = useQueryClient()

  return useMutation<CreateApiKeyResponse, ApiError, CreateApiKeyRequest>({
    mutationFn: async (data) => {
      const response = await api.post('v1/api-keys', { json: data })
      return response.json<CreateApiKeyResponse>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY })
    },
  })
}

export function useUpdateApiKey() {
  const queryClient = useQueryClient()

  return useMutation<ApiKey, ApiError, { keyId: string; name?: string; status?: string }>({
    mutationFn: async ({ keyId, name, status }) => {
      const response = await api.put(`v1/api-keys/${keyId}`, { json: { name, status } })
      return response.json<ApiKey>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY })
    },
  })
}

export function useDeleteApiKey() {
  const queryClient = useQueryClient()

  return useMutation<void, ApiError, string>({
    mutationFn: async (keyId) => {
      await api.delete(`v1/api-keys/${keyId}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: API_KEYS_QUERY_KEY })
    },
  })
}

export function useKeyUsage(range = '30d') {
  return useQuery<KeyUsageResponse, ApiError>({
    queryKey: ['key-usage', range],
    queryFn: async () => {
      const response = await api.get('v1/analytics/keys', {
        searchParams: { range },
      })
      return response.json<KeyUsageResponse>()
    },
  })
}

export { parseApiError }
