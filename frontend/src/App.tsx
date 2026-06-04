import { useEffect } from 'react'
import { Routes, Route, Navigate, useLocation } from 'react-router-dom'
import { useAuth } from '@/hooks/useAuth'
import { DashboardLayout } from '@/components/layout/DashboardLayout'
import Login from '@/pages/Login'
import Dashboard from '@/pages/Dashboard'
import Providers from '@/pages/Providers'
import ApiKeys from '@/pages/ApiKeys'
import Analytics from '@/pages/Analytics'
import Settings from '@/pages/Settings'
import UsersPage from '@/pages/settings/UsersPage'
import ProviderDetailPage from '@/pages/providers/ProviderDetailPage'

function RequireAuth({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isLoading } = useAuth()
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

  useEffect(() => {
    fetchMe().catch(() => {
      // Unauthenticated — handled by RequireAuth
    })
  }, [fetchMe])

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
        <Route path="/settings" element={<Settings />} />
        <Route path="/users" element={<UsersPage />} />
      </Route>
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
  )
}

export default App
