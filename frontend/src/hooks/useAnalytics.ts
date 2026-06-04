import { useQuery } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface TimeSeriesPoint {
  timestamp: string
  requests: number
  tokens: number
  prompt_tokens: number
  completion_tokens: number
  cost_usd: number
  latency_ms: number
  cache_hits: number
  cache_misses: number
}

export interface BreakdownItem {
  dimension: string
  value: string
  requests: number
  tokens: number
  prompt_tokens: number
  completion_tokens: number
  cost_usd: number
}

export interface CacheBreakdownItem {
  model: string
  requests: number
  cache_hits: number
  cache_hit_rate: number
  cost_saved_usd: number
}

export interface AnalyticsData {
  total_requests: number
  total_tokens: number
  prompt_tokens: number
  completion_tokens: number
  total_cost_usd: number
  cost_saved_from_cache_usd: number
  avg_latency_ms: number
  cache_hit_rate: number
  error_rate: number
  time_series: TimeSeriesPoint[]
  by_model: BreakdownItem[]
  by_status: BreakdownItem[]
  top_cached_models: CacheBreakdownItem[]
}

export function useAnalytics(timeRange: string) {
  return useQuery<AnalyticsData, ApiError>({
    queryKey: ['analytics', timeRange],
    queryFn: async () => {
      const response = await api.get(`v1/analytics?range=${timeRange}`)
      return response.json<AnalyticsData>()
    },
    staleTime: 60_000,
  })
}

export { parseApiError }
