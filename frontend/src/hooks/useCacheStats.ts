import { useQuery } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface TopModelStat {
  model_id: string
  entry_count: number
  total_hits: number
  avg_hits: number
}

export interface CacheStatsData {
  org_id: string
  period: string
  hit_rate: number
  cost_saved_usd: number
  entry_count: number
  top_models: TopModelStat[]
}

export function useCacheStats(period: string) {
  return useQuery<CacheStatsData, ApiError>({
    queryKey: ['cache-stats', period],
    queryFn: async () => {
      const response = await api.get(`v1/cache/stats?period=${period}`)
      return response.json<CacheStatsData>()
    },
    staleTime: 60_000,
  })
}

export { parseApiError }
