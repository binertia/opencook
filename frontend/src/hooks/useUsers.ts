import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, parseApiError } from '@/lib/api'
import type { ApiError } from '@/lib/api'

export interface User {
  id: string
  email: string
  name: string
  role: string
  status: 'active' | 'pending' | 'suspended'
  last_login_at: string | null
  created_at: string
}

export interface UsersListResponse {
  object: 'list'
  data: User[]
  pagination: {
    limit: number
    offset: number
    total: number
    has_more: boolean
  }
}

export interface InviteUserRequest {
  email: string
  name: string
  role: 'admin' | 'member' | 'viewer'
  organization_ids?: string[]
}

const USERS_QUERY_KEY = ['users']

export function useUsers(orgId?: string, search?: string, status?: string) {
  return useQuery<UsersListResponse, ApiError>({
    queryKey: [...USERS_QUERY_KEY, orgId, search, status],
    queryFn: async () => {
      const url = new URL('v1/users', window.location.origin)
      if (orgId) url.searchParams.set('org_id', orgId)
      if (search) url.searchParams.set('search', search)
      if (status && status !== 'all') url.searchParams.set('status', status)
      const response = await api.get(url.pathname + url.search)
      return response.json<UsersListResponse>()
    },
  })
}

export function useInviteUser() {
  const queryClient = useQueryClient()

  return useMutation<User, ApiError, InviteUserRequest>({
    mutationFn: async (data) => {
      const response = await api.post('v1/users', { json: data })
      return response.json<User>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USERS_QUERY_KEY })
    },
  })
}

export function useUpdateUserRole() {
  const queryClient = useQueryClient()

  return useMutation<User, ApiError, { userId: string; role: string }>({
    mutationFn: async ({ userId, role }) => {
      const response = await api.put(`v1/users/${userId}`, { json: { role } })
      return response.json<User>()
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USERS_QUERY_KEY })
    },
  })
}

export function useRemoveUser() {
  const queryClient = useQueryClient()

  return useMutation<void, ApiError, string>({
    mutationFn: async (userId) => {
      await api.delete(`v1/users/${userId}`)
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: USERS_QUERY_KEY })
    },
  })
}

export { parseApiError }
