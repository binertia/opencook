import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface RequestItem {
  id: string
  trace_id: string
  model_requested: string | null
  model_routed: string | null
  status: string
  status_code: number | null
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  total_cost: string
  latency_total_ms: number | null
  cache_hit: boolean
  gateway_received_at: string | null
  completed_at: string | null
  error_message: string | null
  provider: string | null
}

export interface RequestsResponse {
  data: RequestItem[]
  total: number
  limit: number
  offset: number
}

export interface LogFilters {
  search?: string
  status?: string
  model?: string
  provider?: string
  dateFrom?: string
  dateTo?: string
}

export function useRequestLogs(limit = 50, offset = 0) {
  return useQuery<RequestsResponse, ApiError>({
    queryKey: ['requests', limit, offset],
    queryFn: async () => {
      const response = await api.get('v1/requests', {
        searchParams: { limit: String(limit), offset: String(offset) },
      })
      return response.json<RequestsResponse>()
    },
    refetchInterval: 30_000,
  })
}
