import { useMemo } from 'react'
import { useOrganization, useUpdateOrganization } from './useOrganization'
import { useAnalytics } from './useAnalytics'

export interface BudgetSettings {
  monthly_limit: number | null
  cost_budget_usd: number | null
  alert_threshold_percent: number
}

export interface AlertThreshold {
  percent: number
  label: string
}

export const DEFAULT_THRESHOLDS: AlertThreshold[] = [
  { percent: 50, label: '50%' },
  { percent: 75, label: '75%' },
  { percent: 90, label: '90%' },
  { percent: 100, label: '100%' },
]

export function useBudget() {
  const { data: org, isLoading: orgLoading } = useOrganization()
  const { data: analytics, isLoading: analyticsLoading } = useAnalytics('30d')
  const updateOrg = useUpdateOrganization(org?.id)

  const budget = useMemo(() => {
    if (!org) return null
    return {
      monthly_limit: org.settings.token_budget?.monthly_limit ?? null,
      cost_budget_usd: org.settings.token_budget?.cost_budget_usd ?? null,
      alert_threshold_percent: org.settings.token_budget?.alert_threshold_percent ?? 80,
    }
  }, [org])

  const currentSpend = analytics?.total_cost_usd ?? 0

  const progress = useMemo(() => {
    if (!budget?.cost_budget_usd || budget.cost_budget_usd <= 0) return null
    const pct = (currentSpend / budget.cost_budget_usd) * 100
    return {
      percentage: Math.min(pct, 100),
      rawPercentage: pct,
      current: currentSpend,
      limit: budget.cost_budget_usd,
    }
  }, [budget, currentSpend])

  const projection = useMemo(() => {
    if (!analytics?.time_series || analytics.time_series.length === 0) return null
    const days = analytics.time_series.length || 1
    const dailyAvg = currentSpend / days
    const daysInMonth = 30
    return dailyAvg * daysInMonth
  }, [analytics, currentSpend])

  const updateBudget = async (settings: BudgetSettings) => {
    if (!org) return
    await updateOrg.mutateAsync({
      settings: {
        ...org.settings,
        token_budget: {
          ...org.settings.token_budget,
          monthly_limit: settings.monthly_limit,
          cost_budget_usd: settings.cost_budget_usd,
          alert_threshold_percent: settings.alert_threshold_percent,
        },
      },
    })
  }

  return {
    budget,
    currentSpend,
    progress,
    projection,
    isLoading: orgLoading || analyticsLoading,
    isSaving: updateOrg.isPending,
    updateBudget,
  }
}
