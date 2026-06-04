import { useQuery } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'
import type { RecentRequest } from '@/components/dashboard/RecentRequests'

export interface UsageDataPoint {
  timestamp: string
  requests: number
  tokens: number
  cost_usd: number
  latency_ms: number
  cache_hits: number
}

export interface DashboardData {
  total_requests: number
  total_cost_usd: number
  cache_hit_rate: number
  avg_latency_ms: number
  requests_change: number
  cost_change: number
  cache_change: number
  latency_change: number
  recent_requests: RecentRequest[]
  active_providers: {
    id: string
    name: string
    status: 'healthy' | 'degraded' | 'unhealthy'
    last_check: string
  }[]
}

export function useDashboard(timeRange: string) {
  return useQuery<DashboardData, ApiError>({
    queryKey: ['dashboard', timeRange],
    queryFn: async () => {
      const response = await api.get(`v1/dashboard?range=${timeRange}`)
      return response.json<DashboardData>()
    },
    staleTime: 60_000,
  })
}

export { parseApiError }
