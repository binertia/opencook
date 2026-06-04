import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface RoutingRule {
  id: string
  name: string
  enabled: boolean
  strategy: {
    providers: { provider_id: string; weight: number }[]
  }
}

export interface RoutingRulesResponse {
  object: 'list'
  data: RoutingRule[]
}

export function useRoutingRules() {
  return useQuery<RoutingRulesResponse, ApiError>({
    queryKey: ['routing-rules'],
    queryFn: async () => {
      const response = await api.get('v1/routing-rules')
      return response.json<RoutingRulesResponse>()
    },
  })
}

export function useProviderRoutingRules(providerId: string) {
  const { data, ...rest } = useRoutingRules()

  const rules =
    data?.data.filter((rule) =>
      rule.strategy.providers.some((p) => p.provider_id === providerId)
    ) || []

  return { rules, ...rest }
}
