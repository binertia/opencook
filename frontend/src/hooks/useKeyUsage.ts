import { useQuery } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface KeyUsageItem {
  api_key_id: string
  key_name: string
  key_prefix: string
  key_status: 'active' | 'revoked'
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

export interface KeyUsageTimePoint {
  period_start: string
  requests: number
  cost_usd: number
  tokens: number
}

export interface KeyDetailData {
  key: KeyUsageItem
  time_series: KeyUsageTimePoint[]
  top_models: { model_id: string; requests: number; tokens: number; cost_usd: number }[]
}

function computeRange(range: string): string {
  const now = new Date()
  let start = new Date()
  switch (range) {
    case 'today':
      start.setHours(0, 0, 0, 0)
      break
    case '7d':
      start.setDate(now.getDate() - 7)
      break
    case '30d':
      start.setDate(now.getDate() - 30)
      break
    default:
      start.setDate(now.getDate() - 30)
  }
  return start.toISOString()
}

export function useKeyUsage(timeRange: string) {
  return useQuery<KeyUsageItem[], ApiError>({
    queryKey: ['key-usage', timeRange],
    queryFn: async () => {
      const start = computeRange(timeRange)
      const end = new Date().toISOString()
      const response = await api.get(
        `v1/analytics/keys?range=${timeRange}&start_time=${encodeURIComponent(start)}&end_time=${encodeURIComponent(end)}`
      )
      const data = await response.json<KeyUsageResponse>()
      return data.data
    },
    staleTime: 60_000,
  })
}

// For drill-down, we re-fetch analytics and filter by key + build time series
export function useKeyDetail(apiKeyId: string, timeRange: string) {
  return useQuery<KeyDetailData | null, ApiError>({
    queryKey: ['key-detail', apiKeyId, timeRange],
    queryFn: async () => {
      if (!apiKeyId) return null

      // Fetch key usage and analytics in parallel
      const [keyUsageRes, analyticsRes] = await Promise.all([
        api.get(`v1/analytics/keys?range=${timeRange}`).then((r) => r.json<KeyUsageResponse>()),
        api.get(`v1/analytics?range=${timeRange}`).then((r) => r.json<{
          time_series: {
            timestamp: string
            requests: number
            tokens: number
            cost_usd: number
          }[]
          by_model: {
            value: string
            requests: number
            tokens: number
            cost_usd: number
          }[]
        }>()),
      ])

      const keyItem = keyUsageRes.data.find((k) => k.api_key_id === apiKeyId)
      if (!keyItem) return null

      // Build time series from analytics (we don't have per-key time series yet,
      // so we use the global time series as a proxy)
      const timeSeries: KeyUsageTimePoint[] = analyticsRes.time_series.map((p) => ({
        period_start: p.timestamp,
        requests: p.requests,
        cost_usd: p.cost_usd,
        tokens: p.tokens,
      }))

      // Top models from analytics by_model
      const topModels = analyticsRes.by_model
        .map((m) => ({
          model_id: m.value,
          requests: m.requests,
          tokens: m.tokens,
          cost_usd: m.cost_usd,
        }))
        .sort((a, b) => b.cost_usd - a.cost_usd)

      return {
        key: keyItem,
        time_series: timeSeries,
        top_models: topModels,
      }
    },
    staleTime: 60_000,
    enabled: !!apiKeyId,
  })
}

export { parseApiError }
