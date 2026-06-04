import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'

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
}

export interface RequestsResponse {
  data: RequestItem[]
  total: number
  limit: number
  offset: number
}

export function useRequests() {
  return useQuery<RequestsResponse, Error>({
    queryKey: ['requests'],
    queryFn: async () => {
      const response = await api.get('v1/requests')
      return response.json<RequestsResponse>()
    },
  })
}
