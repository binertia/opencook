import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export interface UserOrg {
  org_id: string
  org_name: string
  role: string
}

export interface User {
  id: string
  email: string
  name: string
  role: string
  permissions: string[]
  organizations?: UserOrg[]
}

interface AuthState {
  user: User | null
  accessToken: string | null
  refreshToken: string | null
  expiresAt: number | null // Unix timestamp (seconds)
  isAuthenticated: boolean
  isLoading: boolean
  setUser: (user: User | null) => void
  setAccessToken: (token: string | null) => void
  setRefreshToken: (token: string | null) => void
  setExpiresAt: (ts: number | null) => void
  setAuthenticated: (value: boolean) => void
  setLoading: (value: boolean) => void
  login: (user: User, accessToken: string, refreshToken: string, expiresIn: number) => void
  logout: () => void
}

export const useAuthStore = create<AuthState>()(
  persist(
    (set) => ({
      user: null,
      accessToken: null,
      refreshToken: null,
      expiresAt: null,
      isAuthenticated: false,
      isLoading: true,

      setUser: (user) => set({ user }),
      setAccessToken: (accessToken) => set({ accessToken }),
      setRefreshToken: (refreshToken) => set({ refreshToken }),
      setExpiresAt: (expiresAt) => set({ expiresAt }),
      setAuthenticated: (isAuthenticated) => set({ isAuthenticated }),
      setLoading: (isLoading) => set({ isLoading }),

      login: (user, accessToken, refreshToken, expiresIn) =>
        set({
          user,
          accessToken,
          refreshToken,
          expiresAt: Math.floor(Date.now() / 1000) + expiresIn,
          isAuthenticated: true,
          isLoading: false,
        }),

      logout: () =>
        set({
          user: null,
          accessToken: null,
          refreshToken: null,
          expiresAt: null,
          isAuthenticated: false,
          isLoading: false,
        }),
    }),
    {
      name: 'gateway-auth',
      partialize: (state) => ({
        user: state.user,
        accessToken: state.accessToken,
        refreshToken: state.refreshToken,
        expiresAt: state.expiresAt,
        isAuthenticated: state.isAuthenticated,
      }),
    }
  )
)
