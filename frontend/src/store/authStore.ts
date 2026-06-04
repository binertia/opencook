import { create } from 'zustand'

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
  isAuthenticated: boolean
  isLoading: boolean
  setUser: (user: User | null) => void
  setAuthenticated: (value: boolean) => void
  setLoading: (value: boolean) => void
  login: (user: User) => void
  logout: () => void
}

export const useAuthStore = create<AuthState>((set) => ({
  user: null,
  isAuthenticated: false,
  isLoading: true,

  setUser: (user) => set({ user }),
  setAuthenticated: (isAuthenticated) => set({ isAuthenticated }),
  setLoading: (isLoading) => set({ isLoading }),

  login: (user) =>
    set({
      user,
      isAuthenticated: true,
      isLoading: false,
    }),

  logout: () =>
    set({
      user: null,
      isAuthenticated: false,
      isLoading: false,
    }),
}))
