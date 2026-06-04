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
  const store = useAuthStore()

  const login = useCallback(
    async (credentials: LoginRequest): Promise<{ success: boolean; error?: string }> => {
      try {
        const response = await api.post('v1/auth/login', {
          json: credentials,
        })
        const data = await response.json<LoginResponse>()
        store.login(data.user)
        return { success: true }
      } catch (error) {
        const apiError = await parseApiError(error)
        return { success: false, error: apiError.message }
      }
    },
    [store]
  )

  const logout = useCallback(async () => {
    try {
      await api.post('v1/auth/logout')
    } catch {
      // Ignore errors on logout
    } finally {
      store.logout()
      navigate('/login')
    }
  }, [store, navigate])

  const refresh = useCallback(async (): Promise<boolean> => {
    try {
      const response = await api.post('v1/auth/refresh')
      const data = await response.json<{ user: User }>()
      store.login(data.user)
      return true
    } catch {
      store.logout()
      return false
    }
  }, [store])

  const fetchMe = useCallback(async (): Promise<boolean> => {
    try {
      const response = await api.get('v1/auth/me')
      const data = await response.json<User>()
      store.login(data)
      return true
    } catch {
      store.logout()
      return false
    }
  }, [store])

  return {
    user: store.user,
    isAuthenticated: store.isAuthenticated,
    isLoading: store.isLoading,
    login,
    logout,
    refresh,
    fetchMe,
  }
}
