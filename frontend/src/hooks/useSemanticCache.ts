import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'

export interface SemanticCacheStats {
  org_id: string
  total_entries: number
  newest_entry: string | null
}

export function useSemanticCacheStats(orgId?: string) {
  return useQuery<SemanticCacheStats, Error>({
    queryKey: ['semantic-cache-stats', orgId],
    queryFn: async () => {
      const res = await api.get(`api/v1/cache/semantic-stats`)
      return res.json<SemanticCacheStats>()
    },
    enabled: !!orgId,
  })
}
