import { useState, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { ArrowLeft, Save, TrendingUp, AlertCircle } from 'lucide-react'
import { useBudget, DEFAULT_THRESHOLDS } from '@/hooks/useBudget'
import { BudgetProgressBar } from '@/components/budget/BudgetProgressBar'
import { AlertHistory } from '@/components/budget/AlertHistory'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'


// Mock alert history until backend supports it
const MOCK_ALERTS = [
  {
    id: '1',
    timestamp: new Date(Date.now() - 86400000 * 2).toISOString(),
    threshold_percent: 75,
    current_spend_usd: 75.5,
    budget_limit_usd: 100,
    notification_sent: true,
    channel: 'dashboard' as const,
  },
  {
    id: '2',
    timestamp: new Date(Date.now() - 86400000 * 5).toISOString(),
    threshold_percent: 50,
    current_spend_usd: 52.0,
    budget_limit_usd: 100,
    notification_sent: true,
    channel: 'email' as const,
  },
]

export default function BudgetPage() {
  const { budget, progress, projection, isLoading, isSaving, updateBudget } =
    useBudget()

  const [hasLimit, setHasLimit] = useState(budget?.cost_budget_usd != null)
  const [limitInput, setLimitInput] = useState(
    budget?.cost_budget_usd?.toString() ?? ''
  )
  const [thresholds, setThresholds] = useState<number[]>(
    DEFAULT_THRESHOLDS.map((t) => t.percent)
  )

  // Sync local state when budget data loads
  useState(() => {
    if (budget) {
      setHasLimit(budget.cost_budget_usd != null)
      setLimitInput(budget.cost_budget_usd?.toString() ?? '')
    }
  })

  const handleSave = useCallback(async () => {
    const limit = hasLimit ? parseFloat(limitInput) || 0 : null
    await updateBudget({
      monthly_limit: limit,
      cost_budget_usd: limit,
      alert_threshold_percent: Math.min(...thresholds),
    })
  }, [hasLimit, limitInput, thresholds, updateBudget])

  const toggleThreshold = (percent: number) => {
    setThresholds((prev) =>
      prev.includes(percent)
        ? prev.filter((p) => p !== percent)
        : [...prev, percent].sort((a, b) => a - b)
    )
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-4">
          <Button variant="ghost" size="sm" asChild>
            <Link to="/analytics">
              <ArrowLeft className="mr-2 h-4 w-4" />
              Back
            </Link>
          </Button>
          <div>
            <h1 className="text-3xl font-bold tracking-tight">Budget & Alerts</h1>
            <p className="text-muted-foreground">
              Configure budget caps and alert thresholds.
            </p>
          </div>
        </div>
        <Button onClick={handleSave} disabled={isSaving}>
          <Save className="mr-2 h-4 w-4" />
          {isSaving ? 'Saving…' : 'Save'}
        </Button>
      </div>

      {/* Analytics Tabs */}
      <Tabs value="budget" className="w-full">
        <TabsList>
          <TabsTrigger value="overview" asChild>
            <Link to="/analytics">Overview</Link>
          </TabsTrigger>
          <TabsTrigger value="tokens" asChild>
            <Link to="/analytics/tokens">Tokens</Link>
          </TabsTrigger>
          <TabsTrigger value="cache" asChild>
            <Link to="/analytics/cache">Cache</Link>
          </TabsTrigger>
          <TabsTrigger value="keys" asChild>
            <Link to="/analytics/keys">Keys</Link>
          </TabsTrigger>
          <TabsTrigger value="budget" asChild>
            <Link to="/analytics/budget">Budget</Link>
          </TabsTrigger>
        </TabsList>
      </Tabs>

      {/* Current Usage */}
      <Card>
        <CardHeader>
          <CardTitle>Current Usage</CardTitle>
          <CardDescription>
            Spending against your configured budget cap.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {isLoading ? (
            <div className="space-y-4">
              <Skeleton className="h-8 w-full" />
              <Skeleton className="h-4 w-full" />
            </div>
          ) : progress ? (
            <BudgetProgressBar
              percentage={progress.percentage}
              current={progress.current}
              limit={progress.limit}
            />
          ) : (
            <div className="flex items-center gap-3 rounded-md border p-4 text-muted-foreground">
              <AlertCircle className="h-5 w-5" />
              <p>No budget limit configured. Set a cap below to start tracking.</p>
            </div>
          )}

          {projection != null && (
            <div className="flex items-center gap-3 rounded-md bg-muted p-4">
              <TrendingUp className="h-5 w-5 text-muted-foreground" />
              <div>
                <p className="text-sm font-medium">Monthly Projection</p>
                <p className="text-sm text-muted-foreground">
                  At your current rate, you will spend{' '}
                  <span className="font-semibold text-foreground">
                    {new Intl.NumberFormat('en-US', {
                      style: 'currency',
                      currency: 'USD',
                    }).format(projection)}
                  </span>{' '}
                  by month end.
                </p>
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <div className="grid gap-6 lg:grid-cols-2">
        {/* Budget Settings */}
        <Card>
          <CardHeader>
            <CardTitle>Budget Cap</CardTitle>
            <CardDescription>
              Set a monthly spending limit. Requests are blocked when exceeded.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="flex items-center justify-between">
              <div className="space-y-0.5">
                <Label htmlFor="budget-toggle">Enable budget cap</Label>
                <p className="text-xs text-muted-foreground">
                  Turn off for unlimited spending.
                </p>
              </div>
              <Switch
                id="budget-toggle"
                checked={hasLimit}
                onCheckedChange={setHasLimit}
              />
            </div>

            {hasLimit && (
              <div className="space-y-2">
                <Label htmlFor="budget-limit">Monthly limit (USD)</Label>
                <Input
                  id="budget-limit"
                  type="number"
                  min={0}
                  step={0.01}
                  placeholder="1000.00"
                  value={limitInput}
                  onChange={(e) => setLimitInput(e.target.value)}
                />
              </div>
            )}


            <div className="space-y-3">
              <Label>Alert Thresholds</Label>
              <p className="text-xs text-muted-foreground">
                Select which thresholds trigger alerts.
              </p>
              <div className="grid grid-cols-2 gap-3">
                {DEFAULT_THRESHOLDS.map((t) => (
                  <div
                    key={t.percent}
                    className="flex items-center justify-between rounded-md border p-3"
                  >
                    <span className="text-sm font-medium">{t.label}</span>
                    <Switch
                      checked={thresholds.includes(t.percent)}
                      onCheckedChange={() => toggleThreshold(t.percent)}
                    />
                  </div>
                ))}
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Alert History */}
        <Card>
          <CardHeader>
            <CardTitle>Alert History</CardTitle>
            <CardDescription>
              Past budget alerts and notifications.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <AlertHistory alerts={MOCK_ALERTS} />
          </CardContent>
        </Card>
      </div>
    </div>
  )
}
