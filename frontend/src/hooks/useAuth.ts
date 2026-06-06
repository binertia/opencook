import { useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { api, parseApiError } from '@/lib/api'
import { useAuthStore, type User } from '@/store/authStore'

interface LoginRequest {
  email: string
  password: string
}

interface LoginResponse {
  access_token: string
  refresh_token: string
  token_type: string
  expires_in: number
  user: User
}

export function useAuth() {
  const navigate = useNavigate()
  const user = useAuthStore((s) => s.user)
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated)
  const isLoading = useAuthStore((s) => s.isLoading)
  const loginAction = useAuthStore((s) => s.login)
  const logoutAction = useAuthStore((s) => s.logout)
  const setUser = useAuthStore((s) => s.setUser)
  const setAuthenticated = useAuthStore((s) => s.setAuthenticated)
  const setLoading = useAuthStore((s) => s.setLoading)
  const refreshToken = useAuthStore((s) => s.refreshToken)

  const login = useCallback(
    async (credentials: LoginRequest): Promise<{ success: boolean; error?: string }> => {
      try {
        const response = await api.post('v1/auth/login', {
          json: credentials,
        })
        const data = await response.json<LoginResponse>()
        loginAction(data.user, data.access_token, data.refresh_token, data.expires_in)
        return { success: true }
      } catch (error) {
        const apiError = await parseApiError(error)
        return { success: false, error: apiError.message }
      }
    },
    [loginAction]
  )

  const logout = useCallback(async () => {
    const token = useAuthStore.getState().refreshToken
    try {
      if (token) {
        await api.post('v1/auth/logout', { json: { refresh_token: token } })
      }
    } catch {
      // Ignore errors on logout
    } finally {
      logoutAction()
      navigate('/login')
    }
  }, [logoutAction, navigate])

  const refresh = useCallback(async (): Promise<boolean> => {
    const token = useAuthStore.getState().refreshToken
    if (!token) {
      logoutAction()
      return false
    }
    try {
      const response = await api.post('v1/auth/refresh', {
        json: { refresh_token: token },
      })
      const data = await response.json<LoginResponse>()
      loginAction(data.user, data.access_token, data.refresh_token, data.expires_in)
      return true
    } catch {
      logoutAction()
      return false
    }
  }, [loginAction, logoutAction])

  const fetchMe = useCallback(async (): Promise<boolean> => {
    try {
      const response = await api.get('v1/auth/me')
      const data = await response.json<User>()
      setUser(data)
      setAuthenticated(true)
      setLoading(false)
      return true
    } catch {
      logoutAction()
      return false
    }
  }, [setUser, setAuthenticated, setLoading, logoutAction])

  return {
    user,
    isAuthenticated,
    isLoading,
    login,
    logout,
    refresh,
    fetchMe,
    refreshToken,
  }
}
