import { useEffect, useRef } from 'react'
import { Routes, Route, Navigate, useLocation } from 'react-router-dom'
import { useAuth } from '@/hooks/useAuth'
import { useAuthStore } from '@/store/authStore'
import { useAutoRefresh } from '@/hooks/useAutoRefresh'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import Login from '@/pages/Login'
import Dashboard from '@/pages/Dashboard'
import Providers from '@/pages/Providers'
import ApiKeys from '@/pages/ApiKeys'
import Analytics from '@/pages/Analytics'
import TokenUsagePage from '@/pages/analytics/TokenUsagePage'
import CacheAnalyticsPage from '@/pages/analytics/CacheAnalyticsPage'
import KeyUsagePage from '@/pages/analytics/KeyUsagePage'
import BudgetPage from '@/pages/analytics/BudgetPage'
import RequestsPage from '@/pages/Requests'
import Settings from '@/pages/Settings'
import UsersPage from '@/pages/settings/UsersPage'
import AuditLogPage from '@/pages/settings/AuditLogPage'
import ProviderDetailPage from '@/pages/providers/ProviderDetailPage'
import WebhooksPage from '@/pages/webhooks/WebhooksPage'

function RequireAuth({ children }: { children: React.ReactNode }) {
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const isLoading = useAuthStore((s) => s.isLoading)
  const location = useLocation()

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <p className="text-muted-foreground">Loading...</p>
      </div>
    )
  }

  if (!isAuthenticated) {
    return (
      <Navigate
        to={`/login?returnUrl=${encodeURIComponent(location.pathname + location.search)}`}
        replace
      />
    )
  }

  return <>{children}</>
}

function App() {
  const { fetchMe } = useAuth()
  const location = useLocation()
  const hasChecked = useRef(false)

  useAutoRefresh()

  useEffect(() => {
    if (hasChecked.current) return
    hasChecked.current = true

    if (location.pathname === '/login') {
      useAuthStore.getState().setLoading(false)
      return
    }

    const token = useAuthStore.getState().accessToken
    if (token) {
      fetchMe().catch(() => {
        // Unauthenticated — logout already called inside fetchMe
      })
    } else {
      useAuthStore.getState().setLoading(false)
    }
  }, [fetchMe, location.pathname])

  return (
    <Routes>
      <Route path="/login" element={<Login />} />
      <Route
        element={
          <RequireAuth>
            <DashboardLayout />
          </RequireAuth>
        }
      >
        <Route path="/" element={<Dashboard />} />
        <Route path="/providers" element={<Providers />} />
        <Route path="/providers/:providerId" element={<ProviderDetailPage />} />
        <Route path="/keys" element={<ApiKeys />} />
        <Route path="/analytics" element={<Analytics />} />
        <Route path="/analytics/tokens" element={<TokenUsagePage />} />
        <Route path="/analytics/cache" element={<CacheAnalyticsPage />} />
        <Route path="/analytics/keys" element={<KeyUsagePage />} />
        <Route path="/analytics/budget" element={<BudgetPage />} />
        <Route path="/requests" element={<RequestsPage />} />
        <Route path="/webhooks" element={<WebhooksPage />} />
        <Route path="/settings" element={<Settings />} />
        <Route path="/settings/audit-log" element={<AuditLogPage />} />
        <Route path="/users" element={<UsersPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  )
}

export default App
